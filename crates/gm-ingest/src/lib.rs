//! gm-ingest: model ingestion (`docs/MODELS.md` 3–5). A glTF upload goes in; a report comes out,
//! and, when every check passes, the `.gmm` the hub will serve and the preview a moderator will
//! look at.
//!
//! - [`source`]: the upload, bounds-checked, merged onto the standard rig in model space.
//! - [`texture`]: the atlas: resampled, mipped in linear light, encoded as BC1.
//! - [`envelope`], [`raster`], [`verify`]: re-exported from gm-model, where the hub reads them
//!   without linking a parser of uploads.
//! - [`write`], [`synth`]: writing `.glb` files: the creator's template and generated avatars.
//!
//! The hub never calls [`ingest`] in its own process: uploads are parsed in a worker
//! (`gm-ingest-worker`, a binary of this crate, [`worker`]) so that a hostile file costs one
//! upload and no more; the hub's tests run [`in_process`] instead.
#![forbid(unsafe_code)]

pub mod source;
pub mod synth;
pub mod texture;
pub mod write;

use std::path::Path;

use gm_core::vocab::ArchetypeFrame;
use gm_model::format::{self, FLAG_CUTOUT, FLAG_OVERLAY, FLAG_TWO_SIDED, limits};
use gm_model::{Model, ModelId, rig};
// What the hub needs of an upload's answer without reading an upload lives in gm-model
// (2026-10-15): the report's types, the envelope, the rasteriser and `verify`.
pub use gm_model::report::{Facts, Report, Verified, WORKER_MODEL, WORKER_REPORT, verify};
pub use gm_model::{envelope, raster};

/// The largest upload the pipeline looks at (MODELS.md 3).
pub const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
/// UVs may leave the unit square by this much before they are a violation.
const UV_SLACK: f32 = 0.01;

pub struct Ingested {
    pub id: ModelId,
    /// The `.gmm` file.
    pub gmm: Vec<u8>,
    /// The moderation preview, PNG.
    pub preview: Vec<u8>,
    pub model: Model,
}

/// Validate an upload for `frame` and re-encode it.
pub fn ingest(upload: &[u8], frame: ArchetypeFrame) -> (Report, Option<Ingested>) {
    let mut report = Report {
        frame: rig::frame_name(frame).to_string(),
        ..Default::default()
    };
    if upload.len() > MAX_UPLOAD_BYTES {
        report.violations.push(format!(
            "the upload is {} bytes; the limit is {MAX_UPLOAD_BYTES}",
            upload.len()
        ));
        return (report, None);
    }
    let src = match source::read(upload) {
        Ok(s) => s,
        Err(violations) => {
            report.violations = violations;
            return (report, None);
        }
    };
    report.notes = src.notes.clone();
    let facts = &mut report.facts;
    facts.triangles = src.mesh.triangles() as u32;
    facts.vertices = src.mesh.positions.len() as u32;
    facts.source_joints = src.source_joints as u32;
    facts.bones = src.mesh.bone_mask.count_ones();
    facts.source_texture = [src.texture.width(), src.texture.height()];
    facts.cutout = src.cutout.is_some();
    facts.two_sided = src.two_sided;
    let dropped = src.source_triangles - src.mesh.triangles();
    if dropped > 0 {
        report
            .notes
            .push(format!("{dropped} degenerate triangles were dropped"));
    }

    let v = &mut report.violations;
    if src.mesh.triangles() > limits::MAX_TRIANGLES {
        v.push(format!(
            "{} triangles; the budget is {}",
            src.mesh.triangles(),
            limits::MAX_TRIANGLES
        ));
    }
    let stray = src
        .mesh
        .uvs
        .iter()
        .filter(|uv| {
            uv.iter()
                .any(|c| !c.is_finite() || *c < -UV_SLACK || *c > 1.0 + UV_SLACK)
        })
        .count();
    if stray > 0 {
        v.push(format!(
            "{stray} vertices have texture coordinates outside [0, 1]; the atlas does not tile"
        ));
    }
    envelope::check_pose(&src.mesh.pivots, src.mesh.bone_mask, frame, v);
    if src.mesh.triangles() > limits::MAX_TRIANGLES || src.mesh.triangles() == 0 {
        return (report, None);
    }

    let atlas = texture::build(&src.texture, src.base_color, src.cutout);
    let (scale, vertices) = format::quantize_vertices(
        &src.mesh.positions,
        &src.mesh.normals,
        &src.mesh.uvs,
        &src.mesh.joints,
        &src.mesh.weights,
    );
    let model = Model {
        flags: if src.cutout.is_some() { FLAG_CUTOUT } else { 0 }
            | if src.two_sided { FLAG_TWO_SIDED } else { 0 },
        frame: rig::frame_index(frame),
        scale,
        average: atlas.average,
        bone_mask: src.mesh.bone_mask,
        pivots: src.mesh.pivots.map(|p| p.to_array()),
        vertices,
        indices: src.mesh.indices.iter().map(|i| *i as u16).collect(),
        tex_w: atlas.w,
        tex_h: atlas.h,
        texture: atlas.bc1,
    };
    report.facts.texture = [atlas.w as u32, atlas.h as u32];
    let gmm = match model.encode() {
        Ok(bytes) => bytes,
        Err(e) => {
            // Far out of the box (the scale check) or over the size ceiling.
            report
                .violations
                .push(format!("the model cannot be stored: {e}"));
            return (report, None);
        }
    };
    report.facts.bytes = gmm.len() as u32;
    let measured = envelope::check_model(&model, frame, &mut report.violations);
    report.facts.top = measured.top;
    report.facts.coverage_front = measured.front;
    report.facts.coverage_side = measured.side;
    if !report.violations.is_empty() {
        return (report, None);
    }
    // The last word is the check the hub repeats on the stored bytes: an upload is accepted
    // here exactly when its `.gmm` is accepted there.
    let Verified { id, model, preview } = match verify(&gmm, frame) {
        Ok(v) => v,
        Err(violations) => {
            report.violations = violations;
            return (report, None);
        }
    };
    report.ok = true;
    report.id = format::id_hex(&id);
    (
        report,
        Some(Ingested {
            id,
            gmm,
            preview,
            model,
        }),
    )
}

/// Validate an armour overlay (CONTENT.md 3.3 and 4) made for `frame` and re-encode it: a
/// body's file on the standard rig in the T-pose, under an overlay's budgets (triangles,
/// texture, bytes), with no envelope (it covers part of a body, not all of one) and no
/// preview. The report's `frame` is the frame's name with `overlay` after it.
pub fn ingest_overlay(upload: &[u8], frame: ArchetypeFrame) -> (Report, Option<Ingested>) {
    let mut report = Report {
        frame: format!("{} overlay", rig::frame_name(frame)),
        ..Default::default()
    };
    if upload.len() > MAX_UPLOAD_BYTES {
        report.violations.push(format!(
            "the upload is {} bytes; the limit is {MAX_UPLOAD_BYTES}",
            upload.len()
        ));
        return (report, None);
    }
    let src = match source::read(upload) {
        Ok(s) => s,
        Err(violations) => {
            report.violations = violations;
            return (report, None);
        }
    };
    report.notes = src.notes.clone();
    let facts = &mut report.facts;
    facts.triangles = src.mesh.triangles() as u32;
    facts.vertices = src.mesh.positions.len() as u32;
    facts.source_joints = src.source_joints as u32;
    facts.bones = src.mesh.bone_mask.count_ones();
    facts.source_texture = [src.texture.width(), src.texture.height()];
    facts.cutout = src.cutout.is_some();
    facts.two_sided = src.two_sided;
    let v = &mut report.violations;
    if src.mesh.triangles() > limits::OVERLAY_MAX_TRIANGLES {
        v.push(format!(
            "{} triangles; an overlay's budget is {}",
            src.mesh.triangles(),
            limits::OVERLAY_MAX_TRIANGLES
        ));
    }
    envelope::check_pose(&src.mesh.pivots, src.mesh.bone_mask, frame, v);
    if src.mesh.triangles() > limits::OVERLAY_MAX_TRIANGLES || src.mesh.triangles() == 0 {
        return (report, None);
    }
    let atlas = texture::build_capped(
        &src.texture,
        src.base_color,
        src.cutout,
        limits::OVERLAY_MAX_TEXTURE,
    );
    let (scale, vertices) = format::quantize_vertices(
        &src.mesh.positions,
        &src.mesh.normals,
        &src.mesh.uvs,
        &src.mesh.joints,
        &src.mesh.weights,
    );
    let model = Model {
        flags: FLAG_OVERLAY
            | if src.cutout.is_some() { FLAG_CUTOUT } else { 0 }
            | if src.two_sided { FLAG_TWO_SIDED } else { 0 },
        frame: rig::frame_index(frame),
        scale,
        average: atlas.average,
        bone_mask: src.mesh.bone_mask,
        pivots: src.mesh.pivots.map(|p| p.to_array()),
        vertices,
        indices: src.mesh.indices.iter().map(|i| *i as u16).collect(),
        tex_w: atlas.w,
        tex_h: atlas.h,
        texture: atlas.bc1,
    };
    report.facts.texture = [atlas.w as u32, atlas.h as u32];
    let gmm = match model.encode() {
        Ok(bytes) => bytes,
        Err(e) => {
            report
                .violations
                .push(format!("the overlay cannot be stored: {e}"));
            return (report, None);
        }
    };
    if gmm.len() > limits::OVERLAY_MAX_FILE_BYTES {
        report.violations.push(format!(
            "the overlay is {} bytes; the limit is {}",
            gmm.len(),
            limits::OVERLAY_MAX_FILE_BYTES
        ));
        return (report, None);
    }
    report.facts.bytes = gmm.len() as u32;
    let top = (0..model.vertices.len())
        .map(|i| model.position(i).z)
        .fold(0.0f32, f32::max);
    report.facts.top = top;
    let model = match Model::decode(&gmm) {
        Ok(m) => m,
        Err(e) => {
            report
                .violations
                .push(format!("the overlay does not read back: {e}"));
            return (report, None);
        }
    };
    let id = format::model_id(&gmm);
    report.ok = true;
    report.id = format::id_hex(&id);
    (
        report,
        Some(Ingested {
            id,
            gmm,
            preview: Vec::new(),
            model,
        }),
    )
}

/// Validate a prop's file (CONTENT.md 3.1 and 4) under `fit` and re-encode it. The report's
/// `frame` says `prop`; there is no envelope and no preview, and the facts' coverages are 0.
pub fn ingest_prop(upload: &[u8], fit: source::Fit) -> (Report, Option<Ingested>) {
    let mut report = Report {
        frame: "prop".to_string(),
        ..Default::default()
    };
    if upload.len() > MAX_UPLOAD_BYTES {
        report.violations.push(format!(
            "the upload is {} bytes; the limit is {MAX_UPLOAD_BYTES}",
            upload.len()
        ));
        return (report, None);
    }
    let src = match source::read_prop(upload, fit) {
        Ok(s) => s,
        Err(violations) => {
            report.violations = violations;
            return (report, None);
        }
    };
    report.notes = src.notes.clone();
    let facts = &mut report.facts;
    facts.triangles = src.mesh.triangles() as u32;
    facts.vertices = src.mesh.positions.len() as u32;
    facts.source_joints = src.source_joints as u32;
    facts.bones = 1;
    facts.source_texture = [src.texture.width(), src.texture.height()];
    facts.cutout = src.cutout.is_some();
    facts.two_sided = src.two_sided;
    let dropped = src.source_triangles - src.mesh.triangles();
    if dropped > 0 {
        report
            .notes
            .push(format!("{dropped} degenerate triangles were dropped"));
    }
    let v = &mut report.violations;
    if src.mesh.triangles() > limits::PROP_MAX_TRIANGLES {
        v.push(format!(
            "{} triangles; a prop's budget is {}",
            src.mesh.triangles(),
            limits::PROP_MAX_TRIANGLES
        ));
    }
    let stray = src
        .mesh
        .uvs
        .iter()
        .filter(|uv| {
            uv.iter()
                .any(|c| !c.is_finite() || *c < -UV_SLACK || *c > 1.0 + UV_SLACK)
        })
        .count();
    if stray > 0 {
        v.push(format!(
            "{stray} vertices have texture coordinates outside [0, 1]; the atlas does not tile"
        ));
    }
    let reach = src
        .mesh
        .positions
        .iter()
        .fold(0f32, |m, p| m.max(p.x.abs()).max(p.y.abs()).max(p.z.abs()));
    report.facts.top = reach;
    if reach > limits::PROP_MAX_EXTENT {
        v.push(format!(
            "the prop reaches {reach:.1} units from its grip; the limit is {} (fit it, or it is no hand-held thing)",
            limits::PROP_MAX_EXTENT
        ));
    }
    if !v.is_empty() || src.mesh.triangles() == 0 {
        if src.mesh.triangles() == 0 {
            v.push("the file has no triangles".into());
        }
        return (report, None);
    }
    let atlas = texture::build_capped(
        &src.texture,
        src.base_color,
        src.cutout,
        limits::PROP_MAX_TEXTURE,
    );
    let n = src.mesh.positions.len();
    let (scale, vertices) = format::quantize_vertices(
        &src.mesh.positions,
        &src.mesh.normals,
        &src.mesh.uvs,
        &vec![[0u8; 4]; n],
        &vec![[1.0, 0.0, 0.0, 0.0]; n],
    );
    let model = Model {
        flags: format::FLAG_PROP
            | if src.cutout.is_some() { FLAG_CUTOUT } else { 0 }
            | if src.two_sided { FLAG_TWO_SIDED } else { 0 },
        frame: format::PROP_FRAME,
        scale,
        average: atlas.average,
        bone_mask: 1,
        pivots: [[0.0; 3]; rig::BONES],
        vertices,
        indices: src.mesh.indices.iter().map(|i| *i as u16).collect(),
        tex_w: atlas.w,
        tex_h: atlas.h,
        texture: atlas.bc1,
    };
    report.facts.texture = [atlas.w as u32, atlas.h as u32];
    let gmm = match model.encode() {
        Ok(bytes) => bytes,
        Err(e) => {
            report
                .violations
                .push(format!("the prop cannot be stored: {e}"));
            return (report, None);
        }
    };
    report.facts.bytes = gmm.len() as u32;
    // The strict reader's word, as for an avatar: what is written is what is read.
    let model = match Model::decode(&gmm) {
        Ok(m) if m.is_prop() && m.encode().ok().as_deref() == Some(&gmm[..]) => m,
        Ok(_) => {
            report
                .violations
                .push("the prop is not in its canonical encoding".into());
            return (report, None);
        }
        Err(e) => {
            report
                .violations
                .push(format!("the prop cannot be stored: {e}"));
            return (report, None);
        }
    };
    let id = format::model_id(&gmm);
    report.ok = true;
    report.id = format::id_hex(&id);
    (
        report,
        Some(Ingested {
            id,
            gmm,
            preview: Vec::new(),
            model,
        }),
    )
}

/// The in-process form of the worker for tests (`gm_hub::IngestMode::InProcess`): the same
/// answer the hub reads from a worker's files.
pub fn in_process(upload: &[u8], frame: ArchetypeFrame) -> (Report, Option<Vec<u8>>) {
    let (report, ingested) = ingest(upload, frame);
    (report, ingested.map(|i| i.gmm))
}

/// The body of the ingestion worker process: read the upload, ingest it, write the report and
/// (when accepted) the model and its preview into `out_dir`. The caller applies the resource
/// limits and the wall clock (MODELS.md 6.2).
pub fn worker(input: &Path, frame: ArchetypeFrame, out_dir: &Path) -> std::io::Result<Report> {
    let upload = std::fs::read(input)?;
    let (report, ingested) = ingest(&upload, frame);
    if let Some(i) = &ingested {
        std::fs::write(out_dir.join(WORKER_MODEL), &i.gmm)?;
    }
    // The report goes last: its presence says the worker finished.
    let json = serde_json::to_vec_pretty(&report).map_err(std::io::Error::other)?;
    std::fs::write(out_dir.join(WORKER_REPORT), json)?;
    Ok(report)
}
