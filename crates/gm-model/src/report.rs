//! What the hub reads of an upload's answer without reading an upload (MODELS.md 4 and 6.2):
//! the report's types, and `verify`, which believes a worker's `.gmm` only as far as its own
//! bytes prove. Here, not in gm-ingest, since 2026-10-15: the hub links no parser of uploads.

use gm_core::vocab::ArchetypeFrame;
use serde::{Deserialize, Serialize};

use crate::{Model, ModelId, envelope, format, png, raster, rig};

/// What was measured, accepted or not.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub triangles: u32,
    pub vertices: u32,
    pub source_joints: u32,
    pub bones: u32,
    pub source_texture: [u32; 2],
    pub texture: [u32; 2],
    /// Size of the `.gmm`.
    pub bytes: u32,
    pub cutout: bool,
    pub two_sided: bool,
    /// Highest vertex, world units.
    pub top: f32,
    /// Coverage relative to the frame's mannequin.
    pub coverage_front: f32,
    pub coverage_side: f32,
    /// A patch (CONTENT.md 3.3): the blocks of the body's atlas it touches, every level.
    #[serde(default)]
    pub blocks: u32,
}

/// The answer to an upload (MODELS.md 4: every violation at once).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub ok: bool,
    /// The model id in hex when `ok`.
    pub id: String,
    pub frame: String,
    pub facts: Facts,
    pub violations: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug)]
pub struct Verified {
    pub id: ModelId,
    pub model: Model,
    /// The moderation preview, PNG, drawn from the bytes that are stored.
    pub preview: Vec<u8>,
}

/// Check a `.gmm` for `frame` from its own bytes alone (MODELS.md 6.2). The ingestion worker
/// parses an untrusted file and may be subverted by it, so the hub believes nothing the
/// worker says: what it returns is read with the strict reader, must be the canonical encoding
/// of what it decodes to (one model, one id), must be for the frame and pass the pose and
/// envelope rules, and the preview a moderator judges it by is drawn here, from these bytes.
/// Nothing in this function reads an upload.
pub fn verify(gmm: &[u8], frame: ArchetypeFrame) -> Result<Verified, Vec<String>> {
    let model = Model::decode(gmm).map_err(|e| vec![format!("the model cannot be stored: {e}")])?;
    let mut v = Vec::new();
    if model.frame != rig::frame_index(frame) {
        v.push(format!(
            "the model is for another frame than {}",
            rig::frame_name(frame)
        ));
    }
    if model.encode().ok().as_deref() != Some(gmm) {
        v.push("the model is not in its canonical encoding".to_string());
    }
    let pivots: [glam::Vec3; rig::BONES] = std::array::from_fn(|b| model.pivot(b));
    envelope::check_pose(&pivots, model.bone_mask, frame, &mut v);
    envelope::check_model(&model, frame, &mut v);
    if !v.is_empty() {
        return Err(v);
    }
    let (_, h) = frame.capsule();
    let preview = png::encode(
        (raster::PREVIEW_SIDE * 2) as u32,
        raster::PREVIEW_SIDE as u32,
        &raster::preview(&model, h),
    );
    Ok(Verified {
        id: format::model_id(gmm),
        model,
        preview,
    })
}

/// The files a worker leaves in its directory: the report last (its presence says the worker
/// finished), and the model when the upload was accepted.
pub const WORKER_REPORT: &str = "report.json";
pub const WORKER_MODEL: &str = "model.gmm";
/// The ETC2 variant beside the model (MODELS.md 5.2), when the upload has one.
pub const WORKER_ETC: &str = "model.etc.gmm";

/// What an accepted upload is: the `.gmm` whose hash is the id, and the ETC2 variant of it
/// when there is one (none for a cutout model).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variants {
    pub gmm: Vec<u8>,
    pub etc: Option<Vec<u8>>,
}

/// Check a derived variant against the model it claims to come from (MODELS.md 6.2): it is
/// read with the strict reader, is in its canonical encoding, is in `format`, names `id`
/// as its base, and is the same model but for its atlas. Its texels are not compared to
/// the original's: they are the worker's encoding of the same resampled image, and the
/// worst a bad encoding can do is look wrong on phones.
pub fn verify_variant(
    original: &Model,
    id: &ModelId,
    format: crate::format::TextureFormat,
    file: &[u8],
) -> Result<Model, Vec<String>> {
    let model =
        Model::decode(file).map_err(|e| vec![format!("the variant cannot be stored: {e}")])?;
    let mut v = Vec::new();
    if model.texture_format != format {
        v.push(format!(
            "the variant is {}, not {}",
            model.texture_format.name(),
            format.name()
        ));
    }
    if model.base.as_ref() != Some(id) {
        v.push("the variant names another model as its base".to_string());
    }
    if !original.same_but_atlas(&model) {
        v.push("the variant is not the model with another atlas".to_string());
    }
    if model.encode().ok().as_deref() != Some(file) {
        v.push("the variant is not in its canonical encoding".to_string());
    }
    if v.is_empty() { Ok(model) } else { Err(v) }
}
