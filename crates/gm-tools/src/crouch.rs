//! `gm-tools map crouch`: the crouched hull (MODES.md 3.5) merged into a compiled map.
//!
//! qbsp builds the clip hulls a target game names and no others. Quake's are a point, the
//! player (32 × 32 × 56) and the large hull; Hexen II's hull 3 is a crouching player,
//! (-16, -16, -12) to (16, 16, 16): 32 × 32 × 28, half the player and as wide. So the map
//! is compiled a second time for Hexen II, and hull 3's clip trees (with the planes they
//! cut along) are appended to the Quake map; each model's fourth head node, which Quake
//! leaves unused at 0, names its root. The file stays a Quake BSP: the rest is untouched,
//! byte for byte, and an engine that knows no hull 3 never looks at it.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::mapbuild::{Tools, run};

const LUMP_PLANES: usize = 1;
const LUMP_CLIPNODES: usize = 9;
const LUMP_MODELS: usize = 14;
const NUM_LUMPS: usize = 15;
const HEADER_BYTES: usize = 4 + NUM_LUMPS * 8;
const BSP29: u32 = 29;
const BSP2: u32 = u32::from_le_bytes(*b"BSP2");
/// A Quake model: mins, maxs, origin, four head nodes, vis leafs, first face, face count.
const QUAKE_MODEL: usize = 64;
/// A Hexen II model: the same with eight head nodes.
const HEXEN_MODEL: usize = 80;
/// Where a model's head nodes begin: after mins, maxs and origin.
const HEAD_NODES_AT: usize = 36;
/// The hull Hexen II numbers 3, the crouching player's.
const CROUCH: usize = 3;

struct Header {
    bsp2: bool,
    lumps: [(usize, usize); NUM_LUMPS],
}

fn header(bytes: &[u8], what: &str) -> Result<Header> {
    if bytes.len() < HEADER_BYTES {
        bail!("{what}: too short for a BSP header");
    }
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let bsp2 = match word(0) {
        BSP29 => false,
        BSP2 => true,
        other => bail!("{what}: not a Quake BSP (magic {other:#010x})"),
    };
    let mut lumps = [(0, 0); NUM_LUMPS];
    for (i, l) in lumps.iter_mut().enumerate() {
        let (ofs, len) = (word(4 + i * 8) as usize, word(8 + i * 8) as usize);
        if ofs.checked_add(len).is_none_or(|end| end > bytes.len()) {
            bail!("{what}: lump {i} lies outside the file");
        }
        *l = (ofs, len);
    }
    Ok(Header { bsp2, lumps })
}

fn lump<'a>(bytes: &'a [u8], h: &Header, i: usize) -> &'a [u8] {
    let (ofs, len) = h.lumps[i];
    &bytes[ofs..ofs + len]
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn i16_at(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

/// A clip node: its plane and its two children (a node index, or contents when negative).
#[derive(Clone, Copy)]
struct ClipNode {
    plane: i32,
    children: [i32; 2],
}

fn clipnodes(bytes: &[u8], h: &Header, what: &str) -> Result<Vec<ClipNode>> {
    let data = lump(bytes, h, LUMP_CLIPNODES);
    let stride = if h.bsp2 { 12 } else { 8 };
    if !data.len().is_multiple_of(stride) {
        bail!("{what}: the clipnodes lump is not a whole number of nodes");
    }
    Ok(data
        .chunks_exact(stride)
        .map(|c| ClipNode {
            plane: i32_at(c, 0),
            children: if h.bsp2 {
                [i32_at(c, 4), i32_at(c, 8)]
            } else {
                [i16_at(c, 4) as i32, i16_at(c, 6) as i32]
            },
        })
        .collect())
}

/// Every model's head nodes, from a models lump of `stride`-byte models with `count` heads.
fn head_nodes(
    bytes: &[u8],
    h: &Header,
    stride: usize,
    count: usize,
    what: &str,
) -> Result<Vec<Vec<i32>>> {
    let data = lump(bytes, h, LUMP_MODELS);
    if data.is_empty() || !data.len().is_multiple_of(stride) {
        bail!("{what}: the models lump is not a whole number of {stride}-byte models");
    }
    Ok(data
        .chunks_exact(stride)
        .map(|m| {
            (0..count)
                .map(|i| i32_at(m, HEAD_NODES_AT + i * 4))
                .collect()
        })
        .collect())
}

/// What a merge added, for the build's log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Added {
    pub clipnodes: usize,
    pub planes: usize,
}

/// `quake` with the crouched hull of `hexen` (the same map compiled with `qbsp -hexen2`)
/// merged in as hull 3: the clip trees of Hexen II's hull 3 appended to the clipnodes, the
/// planes they cut along appended where the map has no identical one, and each model's
/// fourth head node set to its tree's root.
pub fn merge(quake: &[u8], hexen: &[u8]) -> Result<(Vec<u8>, Added)> {
    let qh = header(quake, "the map")?;
    let hh = header(hexen, "the Hexen II compile")?;
    let end = qh.lumps.iter().map(|&(o, l)| o + l).max().unwrap_or(0);
    if end != quake.len() {
        bail!(
            "the map has {} bytes past its lumps (a BSPX section?): not merged",
            quake.len() - end
        );
    }
    let mut nodes = clipnodes(quake, &qh, "the map")?;
    if nodes.is_empty() {
        bail!("the map has no clip hulls (compiled with -noclip?)");
    }
    let heads = head_nodes(quake, &qh, QUAKE_MODEL, 4, "the map")?;
    if heads.iter().any(|h| h[CROUCH] > 0) {
        bail!("the map already carries a crouched hull");
    }
    let hexen_heads = head_nodes(hexen, &hh, HEXEN_MODEL, 8, "the Hexen II compile")?;
    if hexen_heads.len() != heads.len() {
        bail!(
            "the Hexen II compile has {} models, the map {}",
            hexen_heads.len(),
            heads.len()
        );
    }
    let hexen_nodes = clipnodes(hexen, &hh, "the Hexen II compile")?;
    let hexen_planes = lump(hexen, &hh, LUMP_PLANES);
    let mut planes = lump(quake, &qh, LUMP_PLANES).to_vec();
    if !planes.len().is_multiple_of(20) || !hexen_planes.len().is_multiple_of(20) {
        bail!("a planes lump is not a whole number of planes");
    }
    // The map's own planes by their bytes: a plane the trees share is not stored twice.
    let mut known: HashMap<[u8; 20], i32> = planes
        .chunks_exact(20)
        .enumerate()
        .map(|(i, p)| (p.try_into().unwrap(), i as i32))
        .collect();
    let first_plane = known.len();
    let base = nodes.len();
    let mut copied: HashMap<i32, i32> = HashMap::new();
    let mut roots = Vec::with_capacity(heads.len());
    for h in &hexen_heads {
        let root = h[CROUCH];
        if root < 0 {
            roots.push(root);
            continue;
        }
        // Depth first with an explicit stack: a node is numbered when first met, its
        // children are filled in once they are numbered too.
        let mut stack = vec![root];
        while let Some(&n) = stack.last() {
            let node = *hexen_nodes
                .get(n as usize)
                .with_context(|| format!("hull 3 names clip node {n}, past the lump"))?;
            if let std::collections::hash_map::Entry::Vacant(slot) = copied.entry(n) {
                let plane: [u8; 20] = hexen_planes
                    .get(node.plane as usize * 20..node.plane as usize * 20 + 20)
                    .with_context(|| {
                        format!("clip node {n} names plane {}, past the lump", node.plane)
                    })?
                    .try_into()
                    .unwrap();
                let index = *known.entry(plane).or_insert_with(|| {
                    planes.extend_from_slice(&plane);
                    (planes.len() / 20 - 1) as i32
                });
                slot.insert(nodes.len() as i32);
                nodes.push(ClipNode {
                    plane: index,
                    children: node.children,
                });
            }
            match node
                .children
                .iter()
                .find(|&&c| c >= 0 && !copied.contains_key(&c))
            {
                Some(&c) => stack.push(c),
                None => {
                    stack.pop();
                }
            }
        }
        roots.push(copied[&root]);
    }
    // The children were the Hexen II numbers: renumber them into the map's.
    for node in &mut nodes[base..] {
        for c in &mut node.children {
            if *c >= 0 {
                *c = copied[&*c];
            }
        }
    }
    if !qh.bsp2 && nodes.len() > i16::MAX as usize {
        bail!(
            "{} clip nodes with the crouched hull: past BSP29's {}; build the map with --bsp2",
            nodes.len(),
            i16::MAX
        );
    }
    let mut node_bytes = Vec::with_capacity(nodes.len() * 12);
    for n in &nodes {
        node_bytes.extend_from_slice(&n.plane.to_le_bytes());
        for c in n.children {
            if qh.bsp2 {
                node_bytes.extend_from_slice(&c.to_le_bytes());
            } else {
                node_bytes.extend_from_slice(&(c as i16).to_le_bytes());
            }
        }
    }
    let mut models = lump(quake, &qh, LUMP_MODELS).to_vec();
    for (m, root) in models.chunks_exact_mut(QUAKE_MODEL).zip(&roots) {
        let at = HEAD_NODES_AT + CROUCH * 4;
        m[at..at + 4].copy_from_slice(&root.to_le_bytes());
    }
    // Every lump where it was in the order of the file, the three changed ones rewritten.
    let mut order: Vec<usize> = (0..NUM_LUMPS).collect();
    order.sort_by_key(|&i| qh.lumps[i].0);
    let mut out = vec![0u8; HEADER_BYTES];
    out[0..4].copy_from_slice(&quake[0..4]);
    for i in order {
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let data: &[u8] = match i {
            LUMP_PLANES => &planes,
            LUMP_CLIPNODES => &node_bytes,
            LUMP_MODELS => &models,
            _ => lump(quake, &qh, i),
        };
        let ofs = out.len();
        out[4 + i * 8..8 + i * 8].copy_from_slice(&(ofs as u32).to_le_bytes());
        out[8 + i * 8..12 + i * 8].copy_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }
    let added = Added {
        clipnodes: nodes.len() - base,
        planes: planes.len() / 20 - first_plane,
    };
    Ok((out, added))
}

/// Compile `map` for Hexen II beside `bsp` and merge its crouched hull into `bsp` in place.
pub fn add(map: &Path, bsp: &Path, wad_dir: &Path, tools: &Tools) -> Result<Added> {
    let stem = bsp.file_stem().context("map path has no file name")?;
    let scratch = std::env::temp_dir().join(format!(
        "gm-crouch-{}-{}",
        std::process::id(),
        stem.to_string_lossy()
    ));
    fs::create_dir_all(&scratch)?;
    let result = (|| {
        let hexen = scratch.join(stem).with_extension("bsp");
        let args = [
            "-log",
            "0",
            "-nopercent",
            "-noprogress",
            "-hexen2",
            "-wadpath",
            wad_dir.to_str().context("non-UTF-8 wad dir")?,
            map.to_str().context("non-UTF-8 map path")?,
            hexen.to_str().context("non-UTF-8 scratch path")?,
        ];
        run(&tools.qbsp, &args)?;
        let quake = fs::read(bsp).with_context(|| format!("reading {}", bsp.display()))?;
        let hexen = fs::read(&hexen).with_context(|| format!("reading {}", hexen.display()))?;
        let (merged, added) = merge(&quake, &hexen)?;
        // Checked by the loader before it replaces the map.
        let parsed = gm_bsp::Bsp::parse(&merged).context("the merged map does not load")?;
        if !parsed.has_crouch_hull() {
            bail!("the merged map has no crouched hull");
        }
        let tmp = bsp.with_extension("bsp.crouch");
        fs::write(&tmp, &merged)?;
        fs::rename(&tmp, bsp)?;
        Ok(added)
    })();
    // The scratch holds qbsp's by-products (.prt, .json, the Hexen II map): not kept.
    let _ = fs::remove_dir_all(&scratch);
    let added = result?;
    println!(
        "map crouch: {} ({} clip nodes, {} planes added)",
        bsp.display(),
        added.clipnodes,
        added.planes
    );
    Ok(added)
}
