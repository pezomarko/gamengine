//! Reading an upload (MODELS.md 3): a glTF 2.0 binary evaluated in the pose it was exported
//! in, merged into one mesh in model space on the standard rig. Nothing here trusts the file:
//! every accessor is bounds-checked before a byte is read, and the caller runs in a worker
//! process, so a file that still manages to blow up costs one upload.

use std::collections::HashMap;

use glam::{Mat4, Vec3};
use gltf::accessor::{DataType, Dimensions};
use gltf::mesh::{Mode, Semantic};
use gm_model::mannequin::MeshData;
use gm_model::rig::{self, BONES, bone};

pub const MAX_NODES: usize = 4096;
pub const MAX_SOURCE_JOINTS: usize = 128;
pub const MAX_SOURCE_VERTICES: usize = 200_000;
/// Triangle indices over the whole upload. A mesh may be instanced by every node, so what a
/// small file asks for is bounded here, not by its size.
pub const MAX_SOURCE_INDICES: usize = MAX_SOURCE_VERTICES * 3;
/// Problems listed before the reader stops reading geometry.
const MAX_LISTED: usize = 64;
pub const MAX_SOURCE_TEXTURE: u32 = 4096;
/// Decoded image bytes the worker will allocate at most.
const MAX_IMAGE_ALLOC: u64 = 160 * 1024 * 1024;
/// glTF metres to world units (VOCABULARY.md 2).
const UNITS_PER_METRE: f32 = 32.0;

/// What the upload says, before any budget is applied.
pub struct Source {
    /// Model space, one mesh, joints are standard bones, unused vertices dropped.
    pub mesh: MeshData,
    pub texture: image::RgbaImage,
    pub base_color: [f32; 4],
    /// Alpha cutoff when any material is `MASK` or `BLEND`.
    pub cutout: Option<f32>,
    pub two_sided: bool,
    pub source_joints: usize,
    /// Triangles before degenerate ones were dropped.
    pub source_triangles: usize,
    pub notes: Vec<String>,
}

/// How a prop's file is moved into hand space before anything else (CONTENT.md 3.1): in
/// glTF's own conventions, metres and degrees, applied in this order: scaled, turned about
/// X, then Y, then Z, then moved. The grip ends up at the origin, the business end along
/// +Z (glTF's forward), the edge along +Y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub mov: [f32; 3],
    pub turn: [f32; 3],
    pub scale: f32,
}

impl Default for Fit {
    fn default() -> Fit {
        Fit {
            mov: [0.0; 3],
            turn: [0.0; 3],
            scale: 1.0,
        }
    }
}

impl Fit {
    pub fn matrix(&self) -> Mat4 {
        let r = Mat4::from_rotation_z(self.turn[2].to_radians())
            * Mat4::from_rotation_y(self.turn[1].to_radians())
            * Mat4::from_rotation_x(self.turn[0].to_radians());
        Mat4::from_translation(Vec3::from(self.mov)) * r * Mat4::from_scale(Vec3::splat(self.scale))
    }
}

/// What an upload is read as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A body on the standard rig (MODELS.md 3).
    Avatar,
    /// A thing held (CONTENT.md 3.1): rigid, fitted, on bone 0; a file without a texture
    /// gets one made of swatches of its colours.
    Prop(Fit),
}

/// Swatch cells of a texture made from flat colours: this many texels a side, in a grid
/// of `SWATCH_COLS` columns.
const SWATCH_SIDE: u32 = 8;
const SWATCH_COLS: u32 = 16;
const MAX_SWATCHES: usize = 256;

/// glTF (+Y up, facing +Z, metres) to model space (+Z up, facing +X, units).
fn to_model(p: Vec3) -> Vec3 {
    Vec3::new(p.z, p.x, p.y) * UNITS_PER_METRE
}

fn to_model_dir(n: Vec3) -> Vec3 {
    Vec3::new(n.z, n.x, n.y)
}

/// A bounds-checked view of one accessor.
struct Acc<'a> {
    data: &'a [u8],
    stride: usize,
    count: usize,
    ty: DataType,
    normalized: bool,
}

fn component_bytes(ty: DataType) -> usize {
    match ty {
        DataType::I8 | DataType::U8 => 1,
        DataType::I16 | DataType::U16 => 2,
        DataType::U32 | DataType::F32 => 4,
    }
}

fn accessor<'a>(
    blob: &'a [u8],
    a: &gltf::Accessor<'_>,
    dims: Dimensions,
    types: &[DataType],
    what: &str,
) -> Result<Acc<'a>, String> {
    if a.sparse().is_some() {
        return Err(format!("{what}: sparse accessors are not supported"));
    }
    if a.dimensions() != dims {
        return Err(format!(
            "{what}: expected {dims:?}, found {:?}",
            a.dimensions()
        ));
    }
    let ty = a.data_type();
    if !types.contains(&ty) {
        return Err(format!("{what}: component type {ty:?} is not allowed"));
    }
    let view = a
        .view()
        .ok_or_else(|| format!("{what}: accessor without a buffer view"))?;
    if !matches!(view.buffer().source(), gltf::buffer::Source::Bin) {
        return Err(format!("{what}: data must live in the .glb's BIN chunk"));
    }
    let element = component_bytes(ty) * dims.multiplicity();
    let stride = view.stride().unwrap_or(element);
    if stride < element {
        return Err(format!("{what}: stride smaller than the element"));
    }
    let count = a.count();
    let start = view
        .offset()
        .checked_add(a.offset())
        .ok_or_else(|| format!("{what}: offset overflow"))?;
    let view_end = view
        .offset()
        .checked_add(view.length())
        .ok_or_else(|| format!("{what}: view overflow"))?;
    let end = if count == 0 {
        start
    } else {
        (count - 1)
            .checked_mul(stride)
            .and_then(|v| v.checked_add(start))
            .and_then(|v| v.checked_add(element))
            .ok_or_else(|| format!("{what}: length overflow"))?
    };
    if end > view_end || view_end > blob.len() {
        return Err(format!("{what}: accessor reaches past its buffer"));
    }
    Ok(Acc {
        data: &blob[start..end],
        stride,
        count,
        ty,
        normalized: a.normalized(),
    })
}

impl Acc<'_> {
    fn raw(&self, i: usize, c: usize) -> f64 {
        let at = i * self.stride + c * component_bytes(self.ty);
        let d = &self.data[at..];
        match self.ty {
            DataType::I8 => d[0] as i8 as f64,
            DataType::U8 => d[0] as f64,
            DataType::I16 => i16::from_le_bytes([d[0], d[1]]) as f64,
            DataType::U16 => u16::from_le_bytes([d[0], d[1]]) as f64,
            DataType::U32 => u32::from_le_bytes([d[0], d[1], d[2], d[3]]) as f64,
            DataType::F32 => f32::from_le_bytes([d[0], d[1], d[2], d[3]]) as f64,
        }
    }

    /// The value as a float, with integer types normalised when the accessor says so.
    fn f32(&self, i: usize, c: usize) -> f32 {
        let v = self.raw(i, c);
        let v = if self.normalized {
            match self.ty {
                DataType::I8 => (v / 127.0).max(-1.0),
                DataType::U8 => v / 255.0,
                DataType::I16 => (v / 32767.0).max(-1.0),
                DataType::U16 => v / 65535.0,
                _ => v,
            }
        } else {
            v
        };
        v as f32
    }

    fn u32(&self, i: usize, c: usize) -> u32 {
        self.raw(i, c) as u32
    }

    fn vec3(&self, i: usize) -> Vec3 {
        Vec3::new(self.f32(i, 0), self.f32(i, 1), self.f32(i, 2))
    }
}

struct Graph {
    parent: Vec<Option<usize>>,
    world: Vec<Mat4>,
    /// Nodes that carry a mesh, in traversal order.
    meshes: Vec<usize>,
}

fn graph(doc: &gltf::Document) -> Result<Graph, String> {
    let n = doc.nodes().count();
    if n > MAX_NODES {
        return Err(format!("{n} nodes; the limit is {MAX_NODES}"));
    }
    let mut g = Graph {
        parent: vec![None; n],
        world: vec![Mat4::IDENTITY; n],
        meshes: Vec::new(),
    };
    let mut seen = vec![false; n];
    let mut stack: Vec<(gltf::Node<'_>, Option<usize>)> = Vec::new();
    let scenes: Vec<gltf::Scene<'_>> = match doc.default_scene() {
        Some(s) => vec![s],
        None => doc.scenes().collect(),
    };
    for scene in scenes {
        for node in scene.nodes() {
            stack.push((node, None));
        }
    }
    while let Some((node, parent)) = stack.pop() {
        let i = node.index();
        if std::mem::replace(&mut seen[i], true) {
            return Err("a node is reachable twice (a cycle or an instanced subtree)".into());
        }
        let local = Mat4::from_cols_array_2d(&node.transform().matrix());
        if !local.is_finite() {
            return Err("a node transform is not finite".into());
        }
        g.parent[i] = parent;
        g.world[i] = parent.map_or(local, |p| g.world[p] * local);
        if node.mesh().is_some() {
            g.meshes.push(i);
        }
        for child in node.children() {
            stack.push((child, Some(i)));
        }
    }
    Ok(g)
}

/// Read and merge the upload as an avatar. `Err` lists every reason it cannot be used.
pub fn read(upload: &[u8]) -> Result<Source, Vec<String>> {
    read_kind(upload, Kind::Avatar)
}

/// Read a prop: rigid geometry under `fit`, on bone 0, no rig (CONTENT.md 3.1).
pub fn read_prop(upload: &[u8], fit: Fit) -> Result<Source, Vec<String>> {
    read_kind(upload, Kind::Prop(fit))
}

/// Read and merge the upload. `Err` lists every reason it cannot be used. In order: the
/// file and its node graph, the rig, the geometry primitive by primitive, the atlas, the
/// mesh compacted.
pub fn read_kind(upload: &[u8], kind: Kind) -> Result<Source, Vec<String>> {
    let (gltf, g) = parse(upload)?;
    let blob: &[u8] = gltf.blob.as_deref().unwrap_or(&[]);
    let doc = &gltf.document;
    // A prop has no skin to speak of: whatever the file carries is read as rigid geometry
    // in the pose it was exported in, and the fit moves all of it into hand space.
    let prop_fit = match kind {
        Kind::Avatar => None,
        Kind::Prop(fit) => Some(fit.matrix()),
    };
    let mut reading = Reading::default();
    let rig = read_rig(doc, blob, &g, prop_fit, &mut reading)?;
    let cx = Cx {
        blob,
        g: &g,
        rig: &rig,
        prop_fit,
    };
    reading.read_geometry(doc, &cx);
    let swatched = reading.check(prop_fit.is_some())?;
    let texture = reading.atlas(doc, blob, swatched)?;
    let pivots = pivots(&rig, &g, prop_fit);
    let mask = if prop_fit.is_some() { 1 } else { rig.mask };
    let source_triangles = reading.mesh.indices.len() / 3;
    let mut out = compact(&reading.mesh, pivots, mask);
    if out.normals.contains(&Vec3::ZERO) {
        compute_normals(&mut out);
        reading
            .notes
            .push("normals were missing and have been computed".into());
    }
    Ok(Source {
        mesh: out,
        texture,
        base_color: reading.base_color.unwrap_or([1.0; 4]),
        cutout: reading.cutout,
        two_sided: reading.two_sided,
        source_joints: rig.joints,
        source_triangles,
        notes: reading.notes,
    })
}

/// The file: a glTF 2.0 binary with a node graph the limits allow.
fn parse(upload: &[u8]) -> Result<(gltf::Gltf, Graph), Vec<String>> {
    if !upload.starts_with(b"glTF") {
        return Err(vec!["the upload must be a glTF 2.0 binary (.glb)".into()]);
    }
    let gltf = gltf::Gltf::from_slice(upload)
        .map_err(|e| vec![format!("not a valid glTF 2.0 binary: {e}")])?;
    let g = graph(&gltf.document).map_err(|s| vec![s])?;
    Ok((gltf, g))
}

/// The standard rig as the file has it (MODELS.md 3): which node is which bone, the skin's
/// matrices in the exported pose, the standard bone every joint folds to. A prop has an
/// empty one.
struct Rig {
    node_of_bone: [Option<usize>; BONES],
    bone_of_node: HashMap<usize, usize>,
    /// Joint world × inverse bind, per joint of the skin.
    skin_mats: Vec<Mat4>,
    /// The standard bone each joint's weights go to.
    joint_bone: Vec<usize>,
    joints: usize,
    mask: u32,
}

impl Rig {
    /// The nearest standard bone at or above a node; none when there is none above.
    fn fold(&self, g: &Graph, mut node: usize, include_self: bool) -> Option<usize> {
        if include_self && let Some(b) = self.bone_of_node.get(&node) {
            return Some(*b);
        }
        let mut steps = 0;
        while let Some(p) = g.parent[node] {
            if let Some(b) = self.bone_of_node.get(&p) {
                return Some(*b);
            }
            node = p;
            steps += 1;
            if steps > MAX_NODES {
                break;
            }
        }
        None
    }
}

/// The skin and its joints: exactly one for an avatar, none for a prop.
fn read_rig(
    doc: &gltf::Document,
    blob: &[u8],
    g: &Graph,
    prop_fit: Option<Mat4>,
    reading: &mut Reading,
) -> Result<Rig, Vec<String>> {
    let mut skins = doc.skins();
    let skin = match (skins.next(), skins.next(), prop_fit.is_some()) {
        (_, _, true) => None,
        (Some(skin), None, false) => Some(skin),
        _ => {
            return Err(vec![format!(
                "{} skins; a model has exactly one, on the standard rig",
                doc.skins().count()
            )]);
        }
    };
    let joint_nodes: Vec<gltf::Node<'_>> = skin.iter().flat_map(|s| s.joints()).collect();
    if joint_nodes.len() > MAX_SOURCE_JOINTS {
        return Err(vec![format!(
            "{} joints in the skin; the limit is {MAX_SOURCE_JOINTS}",
            joint_nodes.len()
        )]);
    }
    let mut rig = Rig {
        node_of_bone: [None; BONES],
        bone_of_node: HashMap::new(),
        skin_mats: Vec::new(),
        joint_bone: Vec::new(),
        joints: joint_nodes.len(),
        mask: 0,
    };
    match_bones(&mut rig, &joint_nodes, reading);
    let missing: Vec<&str> = (0..BONES)
        .filter(|b| rig::REQUIRED & (1 << b) != 0 && rig.node_of_bone[*b].is_none())
        .map(|b| rig::NAMES[b])
        .collect();
    if !missing.is_empty() && prop_fit.is_none() {
        reading.violations.push(format!(
            "required bones are missing: {} (joints are matched by name; `gm-tools model template` writes a rig to start from)",
            missing.join(", ")
        ));
        return Err(std::mem::take(&mut reading.violations));
    }
    let folded = joint_nodes
        .iter()
        .filter(|j| !rig.bone_of_node.contains_key(&j.index()))
        .count();
    if folded > 0 {
        reading.notes.push(format!(
            "{folded} joints are not standard bones; their weights went to their nearest standard ancestor"
        ));
    }
    rig.mask = check_hierarchy(&rig, g, reading);
    let ibms = inverse_binds(skin.as_ref(), blob, joint_nodes.len())?;
    rig.skin_mats = joint_nodes
        .iter()
        .zip(&ibms)
        .map(|(j, ibm)| g.world[j.index()] * *ibm)
        .collect();
    if rig.skin_mats.iter().any(|m| !m.is_finite()) {
        return Err(vec!["the skin has non-finite matrices".into()]);
    }
    rig.joint_bone = joint_nodes
        .iter()
        .map(|j| rig.fold(g, j.index(), true).unwrap_or(bone::HIPS))
        .collect();
    Ok(rig)
}

/// Standard bones among the joints, by name.
fn match_bones(rig: &mut Rig, joint_nodes: &[gltf::Node<'_>], reading: &mut Reading) {
    for j in joint_nodes {
        if let Some(b) = j.name().and_then(rig::bone_by_name) {
            if rig.node_of_bone[b].is_some() {
                reading
                    .violations
                    .push(format!("two joints are named `{}`", rig::NAMES[b]));
            }
            rig.node_of_bone[b] = Some(j.index());
            rig.bone_of_node.insert(j.index(), b);
        }
    }
}

/// The file's hierarchy must be the rig's: every bone descends from its nearest present
/// ancestor in the standard rig. Returns the mask of the bones present.
fn check_hierarchy(rig: &Rig, g: &Graph, reading: &mut Reading) -> u32 {
    let mut mask = 0u32;
    for b in 0..BONES {
        let Some(node) = rig.node_of_bone[b] else {
            continue;
        };
        mask |= 1 << b;
        let mut expected = rig::parent(b);
        while let Some(e) = expected
            && rig.node_of_bone[e].is_none()
        {
            expected = rig::parent(e);
        }
        let actual = rig.fold(g, node, false);
        if actual != expected {
            let name = |x: Option<usize>| x.map_or("nothing", |i| rig::NAMES[i]);
            reading.violations.push(format!(
                "`{}` must descend from `{}`; it sits under `{}`",
                rig::NAMES[b],
                name(expected),
                name(actual)
            ));
        }
    }
    mask
}

/// The skin's inverse bind matrices, or the identity for each joint when it has none.
fn inverse_binds(
    skin: Option<&gltf::Skin<'_>>,
    blob: &[u8],
    joints: usize,
) -> Result<Vec<Mat4>, Vec<String>> {
    let Some(a) = skin.and_then(|s| s.inverse_bind_matrices()) else {
        return Ok(vec![Mat4::IDENTITY; joints]);
    };
    let acc = accessor(
        blob,
        &a,
        Dimensions::Mat4,
        &[DataType::F32],
        "inverse bind matrices",
    )
    .map_err(|s| vec![s])?;
    if acc.count != joints {
        return Err(vec![
            "the inverse bind matrices do not match the joints".into(),
        ]);
    }
    Ok((0..acc.count)
        .map(|i| {
            let mut m = [0f32; 16];
            for (c, v) in m.iter_mut().enumerate() {
                *v = acc.f32(i, c);
            }
            Mat4::from_cols_array(&m)
        })
        .collect())
}

/// What every primitive reads against.
struct Cx<'a> {
    blob: &'a [u8],
    g: &'a Graph,
    rig: &'a Rig,
    prop_fit: Option<Mat4>,
}

/// What the primitives add up to, read one after another, and every reason the upload
/// cannot be used so far.
#[derive(Default)]
struct Reading {
    violations: Vec<String>,
    notes: Vec<String>,
    mesh: MeshData,
    /// The images the base colour textures name.
    images: Vec<usize>,
    untextured: usize,
    textured_prims: usize,
    cutout: Option<f32>,
    two_sided: bool,
    base_color: Option<[f32; 4]>,
    /// A prop without a texture: every flat colour (the material's factor times the vertex
    /// colour, when the file has them) becomes a swatch, and the vertex points at it.
    swatches: Vec<[u8; 3]>,
    swatch_uvs: Vec<usize>,
    /// Over the totals: nothing more is read (every further instance would cost the same
    /// again).
    over: bool,
}

/// What a primitive's material and node said before its vertices are read.
struct Prim {
    what: String,
    uv_set: u32,
    swatched: bool,
    skinned: bool,
    rigid_bone: usize,
    /// The node's world matrix (under the fit, for a prop).
    rigid: Mat4,
    factor: [f32; 4],
}

/// A primitive's attributes, bounds-checked, all of one length.
struct Attrs<'a> {
    pos: Acc<'a>,
    nrm: Option<Acc<'a>>,
    uv: Option<Acc<'a>>,
    colours: Option<Acc<'a>>,
    skin: Option<(Acc<'a>, Acc<'a>)>,
    n: usize,
}

impl Reading {
    /// Every mesh node, every primitive, until the limits say stop.
    fn read_geometry(&mut self, doc: &gltf::Document, cx: &Cx<'_>) {
        let nodes: Vec<gltf::Node<'_>> = doc.nodes().collect();
        'nodes: for &ni in &cx.g.meshes {
            let node = &nodes[ni];
            let Some(m) = node.mesh() else { continue };
            let skinned = node.skin().is_some() && cx.prop_fit.is_none();
            let rigid_bone = cx.rig.fold(cx.g, ni, true).unwrap_or(bone::HIPS);
            let rigid = match cx.prop_fit {
                Some(fit) => fit * cx.g.world[ni],
                None => cx.g.world[ni],
            };
            for prim in m.primitives() {
                if self.over || self.violations.len() >= MAX_LISTED {
                    self.violations
                        .push("the rest of the geometry was not read".into());
                    break 'nodes;
                }
                let what = format!("mesh {} primitive {}", m.index(), prim.index());
                if !matches!(
                    prim.mode(),
                    Mode::Triangles | Mode::TriangleStrip | Mode::TriangleFan
                ) {
                    self.violations
                        .push(format!("{what}: only triangles are drawn"));
                    continue;
                }
                let (uv_set, swatched, factor) = self.material(cx, &prim);
                let p = Prim {
                    what,
                    uv_set,
                    swatched,
                    skinned,
                    rigid_bone,
                    rigid,
                    factor,
                };
                if let Err(e) = self.primitive(cx, &prim, &p) {
                    self.violations.push(e);
                }
            }
        }
        if self.mesh.indices.is_empty() && self.violations.is_empty() {
            self.violations.push("the upload has no triangles".into());
        }
    }

    /// The material: its texture (counted, its image listed), the alpha mode, the sides,
    /// the base colour factor. Returns the texture coordinate set, whether the primitive
    /// is swatched (a prop without a texture), and the factor.
    fn material(&mut self, cx: &Cx<'_>, prim: &gltf::Primitive<'_>) -> (u32, bool, [f32; 4]) {
        let material = prim.material();
        let pbr = material.pbr_metallic_roughness();
        let uv_set = match pbr.base_color_texture() {
            Some(info) => {
                let image = info.texture().source().index();
                if !self.images.contains(&image) {
                    self.images.push(image);
                }
                self.textured_prims += 1;
                info.tex_coord()
            }
            None => {
                self.untextured += 1;
                0
            }
        };
        let swatched = cx.prop_fit.is_some() && pbr.base_color_texture().is_none();
        match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => {}
            gltf::material::AlphaMode::Mask => {
                self.cutout
                    .get_or_insert(material.alpha_cutoff().unwrap_or(0.5));
            }
            gltf::material::AlphaMode::Blend => {
                self.cutout.get_or_insert(0.5);
            }
        }
        self.two_sided |= material.double_sided();
        let factor = pbr.base_color_factor();
        const DISAGREE: &str =
            "materials disagree on the base colour factor; the first one is used";
        if *self.base_color.get_or_insert(factor) != factor
            && !self.notes.iter().any(|n| n == DISAGREE)
        {
            self.notes.push(DISAGREE.into());
        }
        (uv_set, swatched, factor)
    }

    /// One primitive's vertices and triangles into the mesh; `Err` is the one reason it
    /// could not be read (the rest of the file still is).
    fn primitive(
        &mut self,
        cx: &Cx<'_>,
        prim: &gltf::Primitive<'_>,
        p: &Prim,
    ) -> Result<(), String> {
        let attrs = attributes(cx.blob, prim, p)?;
        if self.mesh.positions.len() + attrs.n > MAX_SOURCE_VERTICES {
            self.over = true;
            return Err(format!(
                "more than {MAX_SOURCE_VERTICES} vertices in the upload"
            ));
        }
        let base = self.mesh.positions.len() as u32;
        self.vertices(cx, &attrs, p)?;
        self.triangles(cx, prim, &attrs, p, base)
    }

    /// Every vertex of a primitive: bound to its bones in the exported pose, moved to
    /// model space, its texture coordinates (or its swatch).
    fn vertices(&mut self, cx: &Cx<'_>, attrs: &Attrs<'_>, p: &Prim) -> Result<(), String> {
        for i in 0..attrs.n {
            let pos = attrs.pos.vec3(i);
            let nn = attrs.nrm.as_ref().map_or(Vec3::ZERO, |a| a.vec3(i));
            let (m, joints, weights) = match &attrs.skin {
                Some((ja, wa)) => bind(cx.rig, ja, wa, i, p.rigid)
                    .ok_or_else(|| format!("{}: a joint index is past the skin", p.what))?,
                None => (p.rigid, [p.rigid_bone as u8, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
            };
            let world = m.transform_point3(pos);
            if !world.is_finite() {
                return Err(format!("{}: a vertex is not finite", p.what));
            }
            self.mesh.positions.push(to_model(world));
            self.mesh
                .normals
                .push(to_model_dir(m.transform_vector3(nn)).normalize_or_zero());
            if p.swatched {
                let at = self.swatch(attrs.colours.as_ref(), i, p)?;
                self.swatch_uvs.push(at);
                self.mesh.uvs.push([0.0, 0.0]);
            } else {
                let uv = attrs
                    .uv
                    .as_ref()
                    .expect("a textured primitive has coordinates");
                self.mesh.uvs.push([uv.f32(i, 0), uv.f32(i, 1)]);
            }
            self.mesh.joints.push(joints);
            self.mesh.weights.push(weights);
        }
        Ok(())
    }

    /// The vertex's flat colour, linear as glTF gives it, quantised to a swatch (sRGB when
    /// the texture is made, in `atlas`): its index.
    fn swatch(&mut self, colours: Option<&Acc<'_>>, i: usize, p: &Prim) -> Result<usize, String> {
        let mut c = [p.factor[0], p.factor[1], p.factor[2]];
        if let Some(ca) = colours {
            for (k, v) in c.iter_mut().enumerate() {
                *v *= ca.f32(i, k);
            }
        }
        let q = c.map(|v| (linear_to_srgb(v) * 255.0 + 0.5) as u8);
        match self.swatches.iter().position(|s| *s == q) {
            Some(at) => Ok(at),
            None if self.swatches.len() < MAX_SWATCHES => {
                self.swatches.push(q);
                Ok(self.swatches.len() - 1)
            }
            None => Err(format!("{}: more than {MAX_SWATCHES} flat colours", p.what)),
        }
    }

    /// The primitive's triangles, whatever its topology, over `base`.
    fn triangles(
        &mut self,
        cx: &Cx<'_>,
        prim: &gltf::Primitive<'_>,
        attrs: &Attrs<'_>,
        p: &Prim,
        base: u32,
    ) -> Result<(), String> {
        let index: Vec<u32> = match prim.indices() {
            Some(a) => {
                let acc = accessor(
                    cx.blob,
                    &a,
                    Dimensions::Scalar,
                    &[DataType::U8, DataType::U16, DataType::U32],
                    &p.what,
                )?;
                if acc.count > MAX_SOURCE_VERTICES * 3 {
                    return Err(format!("{}: too many indices", p.what));
                }
                let v: Vec<u32> = (0..acc.count).map(|i| acc.u32(i, 0)).collect();
                if v.iter().any(|i| *i as usize >= attrs.n) {
                    return Err(format!("{}: an index is past the vertices", p.what));
                }
                v
            }
            None => (0..attrs.n as u32).collect(),
        };
        // A strip or a fan makes up to three indices of one.
        if self.mesh.indices.len() + index.len() * 3 > MAX_SOURCE_INDICES {
            self.over = true;
            return Err(format!(
                "more than {} triangles in the upload",
                MAX_SOURCE_INDICES / 3
            ));
        }
        let mesh = &mut self.mesh;
        let mut tri = |a: u32, b: u32, c: u32| {
            mesh.indices
                .extend_from_slice(&[base + a, base + b, base + c]);
        };
        match prim.mode() {
            Mode::Triangles => {
                for t in index.chunks_exact(3) {
                    tri(t[0], t[1], t[2]);
                }
            }
            Mode::TriangleStrip => {
                for (k, t) in index.windows(3).enumerate() {
                    if k % 2 == 0 {
                        tri(t[0], t[1], t[2]);
                    } else {
                        tri(t[1], t[0], t[2]);
                    }
                }
            }
            _ => {
                for t in index.windows(2).skip(1) {
                    tri(index[0], t[0], t[1]);
                }
            }
        }
        Ok(())
    }

    /// What the geometry added up to must hold together: triangles, one atlas or swatches
    /// (a prop is textured or flat, not both), and no violation so far. Returns whether
    /// the texture is made of swatches.
    fn check(&mut self, prop: bool) -> Result<bool, Vec<String>> {
        let swatch_texture = match (prop, self.untextured, self.textured_prims) {
            // An avatar: every surface comes from the one atlas.
            (false, u, _) if u > 0 => {
                self.violations.push(format!(
                    "{u} primitives have no base colour texture; every surface comes from the one atlas"
                ));
                false
            }
            // A prop of flat colours: its texture is made here.
            (true, u, 0) if u > 0 => true,
            // A prop of both: the file must choose.
            (true, u, t) if u > 0 && t > 0 => {
                self.violations.push(format!(
                    "{u} primitives have no base colour texture and {t} have one; a prop is textured or flat, not both"
                ));
                false
            }
            _ => false,
        };
        if self.images.len() > 1 {
            self.violations.push(format!(
                "{} base colour textures; the budget is one atlas",
                self.images.len()
            ));
        }
        if !self.violations.is_empty() {
            return Err(std::mem::take(&mut self.violations));
        }
        Ok(swatch_texture)
    }

    /// The atlas: the swatches in a grid (every swatched vertex pointed at the middle of
    /// its cell), or the one image embedded in the file.
    fn atlas(
        &mut self,
        doc: &gltf::Document,
        blob: &[u8],
        swatched: bool,
    ) -> Result<image::RgbaImage, Vec<String>> {
        if swatched {
            return Ok(self.swatch_atlas());
        }
        let image = self
            .images
            .first()
            .and_then(|i| doc.images().nth(*i))
            .ok_or_else(|| vec!["the base colour texture names no image".to_string()])?;
        let bytes = match image.source() {
            gltf::image::Source::View { view, .. } => {
                let end = view.offset().checked_add(view.length());
                match end {
                    Some(end)
                        if end <= blob.len()
                            && matches!(view.buffer().source(), gltf::buffer::Source::Bin) =>
                    {
                        &blob[view.offset()..end]
                    }
                    _ => return Err(vec!["the texture reaches past the .glb's BIN chunk".into()]),
                }
            }
            gltf::image::Source::Uri { .. } => {
                return Err(vec![
                    "the texture must be embedded in the .glb, not referenced by URI".into(),
                ]);
            }
        };
        decode_image(bytes).map_err(|s| vec![s])
    }

    /// The swatches in a grid of `SWATCH_COLS` cells of `SWATCH_SIDE` texels.
    fn swatch_atlas(&mut self) -> image::RgbaImage {
        let rows = (self.swatches.len() as u32).div_ceil(SWATCH_COLS).max(1);
        let (w, h) = (SWATCH_COLS * SWATCH_SIDE, rows * SWATCH_SIDE);
        let mut img = image::RgbaImage::new(w, h);
        for (i, c) in self.swatches.iter().enumerate() {
            let (x0, y0) = (
                (i as u32 % SWATCH_COLS) * SWATCH_SIDE,
                (i as u32 / SWATCH_COLS) * SWATCH_SIDE,
            );
            for y in y0..y0 + SWATCH_SIDE {
                for x in x0..x0 + SWATCH_SIDE {
                    img.put_pixel(x, y, image::Rgba([c[0], c[1], c[2], 255]));
                }
            }
        }
        for (uv, at) in self.mesh.uvs.iter_mut().zip(&self.swatch_uvs) {
            let (col, row) = (*at as u32 % SWATCH_COLS, *at as u32 / SWATCH_COLS);
            *uv = [
                (col as f32 + 0.5) * SWATCH_SIDE as f32 / w as f32,
                (row as f32 + 0.5) * SWATCH_SIDE as f32 / h as f32,
            ];
        }
        // The factor is in the swatches already.
        self.base_color = Some([1.0; 4]);
        self.notes.push(format!(
            "no texture: {} flat colours were made into swatches",
            self.swatches.len()
        ));
        img
    }
}

/// A primitive's attributes, each bounds-checked. A swatched primitive needs no texture
/// coordinates: it gets them. A skinned one needs joints and weights.
fn attributes<'a>(
    blob: &'a [u8],
    prim: &gltf::Primitive<'_>,
    p: &Prim,
) -> Result<Attrs<'a>, String> {
    let what = p.what.as_str();
    let pos = prim
        .get(&Semantic::Positions)
        .ok_or_else(|| format!("{what}: no POSITION"))?;
    let pos = accessor(blob, &pos, Dimensions::Vec3, &[DataType::F32], what)?;
    let nrm = match prim.get(&Semantic::Normals) {
        Some(a) => Some(accessor(
            blob,
            &a,
            Dimensions::Vec3,
            &[DataType::F32],
            what,
        )?),
        None => None,
    };
    let uv = match prim.get(&Semantic::TexCoords(p.uv_set)) {
        Some(a) => Some(accessor(
            blob,
            &a,
            Dimensions::Vec2,
            &[DataType::F32, DataType::U8, DataType::U16],
            what,
        )?),
        None if p.swatched => None,
        None => return Err(format!("{what}: no TEXCOORD_{}", p.uv_set)),
    };
    let colours = match (p.swatched, prim.get(&Semantic::Colors(0))) {
        (true, Some(a)) => {
            let dims = a.dimensions();
            Some(accessor(
                blob,
                &a,
                dims,
                &[DataType::F32, DataType::U8, DataType::U16],
                what,
            )?)
        }
        _ => None,
    };
    let skin = if p.skinned {
        let j = prim
            .get(&Semantic::Joints(0))
            .ok_or_else(|| format!("{what}: no JOINTS_0"))?;
        let w = prim
            .get(&Semantic::Weights(0))
            .ok_or_else(|| format!("{what}: no WEIGHTS_0"))?;
        Some((
            accessor(
                blob,
                &j,
                Dimensions::Vec4,
                &[DataType::U8, DataType::U16],
                what,
            )?,
            accessor(
                blob,
                &w,
                Dimensions::Vec4,
                &[DataType::F32, DataType::U8, DataType::U16],
                what,
            )?,
        ))
    } else {
        None
    };
    let n = pos.count;
    if nrm.as_ref().is_some_and(|a| a.count != n)
        || uv.as_ref().is_some_and(|a| a.count != n)
        || colours.as_ref().is_some_and(|a| a.count != n)
        || skin
            .as_ref()
            .is_some_and(|(j, w)| j.count != n || w.count != n)
    {
        return Err(format!("{what}: attributes differ in length"));
    }
    Ok(Attrs {
        pos,
        nrm,
        uv,
        colours,
        skin,
        n,
    })
}

/// A skinned vertex's bind: the blend of its joints' skin matrices, and its weights folded
/// onto standard bones. An unweighted vertex follows the hips. `None`: a joint index past
/// the skin.
fn bind(
    rig: &Rig,
    ja: &Acc<'_>,
    wa: &Acc<'_>,
    i: usize,
    rigid: Mat4,
) -> Option<(Mat4, [u8; 4], [f32; 4])> {
    let mut m = Mat4::ZERO;
    let mut total = 0.0;
    let mut bones: Vec<(usize, f32)> = Vec::with_capacity(4);
    for c in 0..4 {
        let w = wa.f32(i, c);
        // Skips zero, negative and NaN weights alike.
        if w.is_nan() || w <= 0.0 {
            continue;
        }
        let k = ja.u32(i, c) as usize;
        if k >= rig.skin_mats.len() {
            return None;
        }
        m += rig.skin_mats[k] * w;
        total += w;
        match bones.iter_mut().find(|(b, _)| *b == rig.joint_bone[k]) {
            Some(e) => e.1 += w,
            None => bones.push((rig.joint_bone[k], w)),
        }
    }
    if total <= 0.0 {
        return Some((rigid, [bone::HIPS as u8, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]));
    }
    let mut j = [0u8; 4];
    let mut w = [0f32; 4];
    for (k, (b, bw)) in bones.iter().enumerate() {
        j[k] = *b as u8;
        w[k] = bw / total;
    }
    Some((m * (1.0 / total), j, w))
}

/// Pivots in model space; an absent bone holds its parent's. A prop has none: bone 0 at
/// the origin, where the hand closes.
fn pivots(rig: &Rig, g: &Graph, prop_fit: Option<Mat4>) -> [Vec3; BONES] {
    let mut pivots = [Vec3::ZERO; BONES];
    if prop_fit.is_none() {
        for b in 0..BONES {
            pivots[b] = match rig.node_of_bone[b] {
                Some(n) => to_model(g.world[n].w_axis.truncate()),
                None => rig::parent(b).map_or(Vec3::ZERO, |p| pivots[p]),
            };
        }
    }
    pivots
}

/// The mesh without its degenerate triangles and the vertices nothing uses; vertices keep
/// the order they were uploaded in.
fn compact(mesh: &MeshData, pivots: [Vec3; BONES], mask: u32) -> MeshData {
    let mut kept: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
        let area = (mesh.positions[b] - mesh.positions[a])
            .cross(mesh.positions[c] - mesh.positions[a])
            .length();
        if a != b && b != c && a != c && area > 1e-9 {
            kept.extend_from_slice(t);
        }
    }
    let mut remap = vec![u32::MAX; mesh.positions.len()];
    for i in &kept {
        remap[*i as usize] = 0;
    }
    let mut out = MeshData {
        pivots,
        bone_mask: mask,
        ..Default::default()
    };
    for (old, slot) in remap.iter_mut().enumerate() {
        if *slot == u32::MAX {
            continue;
        }
        *slot = out.positions.len() as u32;
        out.positions.push(mesh.positions[old]);
        out.normals.push(mesh.normals[old]);
        out.uvs.push(mesh.uvs[old]);
        out.joints.push(mesh.joints[old]);
        out.weights.push(mesh.weights[old]);
    }
    for i in &mut kept {
        *i = remap[*i as usize];
    }
    out.indices = kept;
    out
}

/// Linear light to the sRGB curve, 0..1 (a swatch is stored as the texture is).
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * gm_model::det::pow(c, 1.0 / 2.4) - 0.055
    }
}

/// Area-weighted smooth normals for vertices that came without one.
fn compute_normals(mesh: &mut MeshData) {
    let mut sum = vec![Vec3::ZERO; mesh.positions.len()];
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
        let n =
            (mesh.positions[b] - mesh.positions[a]).cross(mesh.positions[c] - mesh.positions[a]);
        sum[a] += n;
        sum[b] += n;
        sum[c] += n;
    }
    for (n, s) in mesh.normals.iter_mut().zip(sum) {
        if *n == Vec3::ZERO {
            *n = s.normalize_or(Vec3::Z);
        }
    }
}

/// Decode a PNG or JPEG with hard limits on its size and on what decoding may allocate.
pub fn decode_image(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("the texture cannot be read: {e}"))?;
    match reader.format() {
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg) => {}
        _ => return Err("the texture must be a PNG or a JPEG".into()),
    }
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("the texture header is invalid: {e}"))?;
    if w == 0 || h == 0 || w > MAX_SOURCE_TEXTURE || h > MAX_SOURCE_TEXTURE {
        return Err(format!(
            "the texture is {w} × {h}; the limit for an upload is {MAX_SOURCE_TEXTURE} × {MAX_SOURCE_TEXTURE}"
        ));
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("the texture cannot be read: {e}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_TEXTURE);
    limits.max_image_height = Some(MAX_SOURCE_TEXTURE);
    limits.max_alloc = Some(MAX_IMAGE_ALLOC);
    reader.limits(limits);
    let img = reader
        .decode()
        .map_err(|e| format!("the texture does not decode: {e}"))?;
    Ok(img.to_rgba8())
}
