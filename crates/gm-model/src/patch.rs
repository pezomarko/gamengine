//! A cloth patch (`.gmp`, MODELS.md 5.1; CONTENT.md 3.3, LOOK.md 6.6): a piece of armour that
//! stays inside the body's silhouette is not a mesh of its own but pixels on the body's own
//! atlas, the way World of Warcraft dressed a character in 2004 and GoldSrc's bodygroups
//! swapped a skin. The file holds, per mip level of the body's atlas, only the 4 × 4 blocks the
//! cloth touches, as BC1 with the one-bit alpha saying which texels are cloth; the client
//! lays them over the body's blocks (`composite`) and draws the dressed body with that one
//! texture, in one draw call. A patch is baked for one body (`body`, the official avatar's
//! key, whose atlas layout it was painted in) and applies to no other.

use crate::bc1;
use crate::format::{ModelError, TEX_BC1_SRGB, bc1_level_bytes, mip_count};

pub const MAGIC: [u8; 4] = *b"GMP1";

pub mod limits {
    /// The whole file: a 1024² patch over a third of the atlas with its mips.
    pub const MAX_FILE_BYTES: usize = 393_216;
    pub const MAX_RAW_BYTES: usize = 2 * 1024 * 1024;
    pub const MAX_BODY_KEY: usize = 48;
}

const HEADER_BYTES: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    /// `rig::frame_index` of the body it was baked for.
    pub frame: u8,
    /// The official avatar's key (`AvatarEntry.key`): the body whose atlas it was baked in.
    pub body: String,
    /// The body's atlas size: the patch is in its texel space.
    pub tex_w: u16,
    pub tex_h: u16,
    /// Per mip level, largest first: the blocks the cloth touches, `(index in the level,
    /// BC1 block)`, by index ascending.
    pub levels: Vec<Vec<(u32, [u8; 8])>>,
}

/// Whether a BC1 block has a transparent texel: the three-colour mode (`c0 <= c1`) with an
/// index of 3 anywhere.
pub fn block_cut(block: &[u8; 8]) -> bool {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    if c0 > c1 {
        return false;
    }
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    (0..16).any(|i| (bits >> (2 * i)) & 3 == 3)
}

/// Whether a block has an opaque texel.
pub fn block_has_cloth(block: &[u8; 8]) -> bool {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    if c0 > c1 {
        return true;
    }
    let bits = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    (0..16).any(|i| (bits >> (2 * i)) & 3 != 3)
}

fn to_565(c: [u8; 4]) -> u16 {
    let r = (c[0] as u32 * 31 + 127) / 255;
    let g = (c[1] as u32 * 63 + 127) / 255;
    let b = (c[2] as u32 * 31 + 127) / 255;
    ((r << 11) | (g << 5) | b) as u16
}

fn dist2(a: [u8; 4], b: [u8; 4]) -> i32 {
    (0..3).map(|k| (a[k] as i32 - b[k] as i32).pow(2)).sum()
}

/// Encode sixteen texels (row by row) as one BC1 block: a range fit, the two texels farthest
/// apart as the endpoints, every other at its nearest palette entry; a texel with alpha 0 is
/// transparent (the three-colour mode). Fast enough to run on a client at a change of
/// clothes, exact enough for the edge blocks of a patch, which are the only ones it meets.
pub fn encode_block(px: &[[u8; 4]; 16]) -> [u8; 8] {
    let opaque: Vec<usize> = (0..16).filter(|&i| px[i][3] != 0).collect();
    if opaque.is_empty() {
        return [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
    }
    let cut = opaque.len() < 16;
    let (mut a, mut b, mut best) = (opaque[0], opaque[0], -1);
    for &i in &opaque {
        for &j in &opaque {
            let d = dist2(px[i], px[j]);
            if d > best {
                best = d;
                a = i;
                b = j;
            }
        }
    }
    let (mut c0, mut c1) = (to_565(px[a]), to_565(px[b]));
    if cut != (c0 <= c1) {
        std::mem::swap(&mut c0, &mut c1);
    }
    let mut out = [0u8; 8];
    out[..2].copy_from_slice(&c0.to_le_bytes());
    out[2..4].copy_from_slice(&c1.to_le_bytes());
    let pal = bc1::palette(&out);
    // Two equal endpoints in the four-colour mode are the three-colour mode to the GPU
    // (`c0 <= c1`): only index 0 is safe there.
    let choices = if cut {
        3
    } else if c0 == c1 {
        1
    } else {
        4
    };
    let mut bits = 0u32;
    for (i, p) in px.iter().enumerate() {
        let idx = if p[3] == 0 {
            3
        } else {
            (0..choices).min_by_key(|&k| dist2(*p, pal[k])).unwrap_or(0) as u32
        };
        bits |= idx << (2 * i);
    }
    out[4..].copy_from_slice(&bits.to_le_bytes());
    out
}

fn valid_side(v: u16) -> bool {
    v.is_power_of_two() && (crate::limits::MIN_TEXTURE..=crate::limits::MAX_TEXTURE).contains(&v)
}

impl Patch {
    /// From a whole BC1 mip chain of the patch's picture over the body's atlas (built as a
    /// cutout, the coverage in the alpha): the blocks with cloth in them, level by level.
    pub fn from_chain(frame: u8, body: &str, tex_w: u16, tex_h: u16, chain: &[u8]) -> Patch {
        let mut levels = Vec::new();
        let mut offset = 0;
        for level in 0..mip_count(tex_w, tex_h) {
            let (w, h) = ((tex_w >> level) as usize, (tex_h >> level) as usize);
            let len = bc1_level_bytes(w, h);
            let blocks = chain[offset..offset + len].as_chunks::<8>().0;
            levels.push(
                blocks
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| block_has_cloth(b))
                    .map(|(i, b)| (i as u32, *b))
                    .collect(),
            );
            offset += len;
        }
        Patch {
            frame,
            body: body.to_string(),
            tex_w,
            tex_h,
            levels,
        }
    }

    /// Blocks in every level together.
    pub fn blocks(&self) -> usize {
        self.levels.iter().map(Vec::len).sum()
    }

    /// Blocks of the largest level: the share of the atlas that is cloth.
    pub fn coverage(&self) -> f32 {
        let all = (self.tex_w as usize).div_ceil(4) * (self.tex_h as usize).div_ceil(4);
        self.levels
            .first()
            .map_or(0.0, |l| l.len() as f32 / all as f32)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.frame > 3 {
            return Err(ModelError::Range("frame"));
        }
        if self.body.is_empty()
            || self.body.len() > limits::MAX_BODY_KEY
            || !self
                .body
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'/')
        {
            return Err(ModelError::Range("body key"));
        }
        if !valid_side(self.tex_w) || !valid_side(self.tex_h) {
            return Err(ModelError::Range("texture size"));
        }
        if self.levels.len() != mip_count(self.tex_w, self.tex_h) as usize {
            return Err(ModelError::Range("mip count"));
        }
        for (level, blocks) in self.levels.iter().enumerate() {
            let (w, h) = (
                (self.tex_w >> level) as usize,
                (self.tex_h >> level) as usize,
            );
            let count = w.div_ceil(4) * h.div_ceil(4);
            let mut last: Option<u32> = None;
            for (i, _) in blocks {
                if *i as usize >= count || last.is_some_and(|l| l >= *i) {
                    return Err(ModelError::Range("block index"));
                }
                last = Some(*i);
            }
        }
        Ok(())
    }

    /// The `.gmp` bytes. Deterministic.
    pub fn encode(&self) -> Result<Vec<u8>, ModelError> {
        self.validate()?;
        let mut raw = Vec::with_capacity(16 + self.blocks() * 12);
        raw.push(self.body.len() as u8);
        raw.extend_from_slice(self.body.as_bytes());
        raw.extend_from_slice(&self.tex_w.to_le_bytes());
        raw.extend_from_slice(&self.tex_h.to_le_bytes());
        raw.push(self.levels.len() as u8);
        raw.extend_from_slice(&[0, 0, 0]);
        for blocks in &self.levels {
            raw.extend_from_slice(&(blocks.len() as u32).to_le_bytes());
            // Gaps between indices, not the indices: small numbers deflate.
            let mut last = 0u32;
            for (i, _) in blocks {
                raw.extend_from_slice(&(*i - last).to_le_bytes());
                last = *i;
            }
            for (_, b) in blocks {
                raw.extend_from_slice(b);
            }
        }
        if raw.len() > limits::MAX_RAW_BYTES {
            return Err(ModelError::TooLarge(raw.len(), limits::MAX_RAW_BYTES));
        }
        let mut out = Vec::with_capacity(HEADER_BYTES + raw.len() / 2);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.push(self.frame);
        out.push(TEX_BC1_SRGB);
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(&miniz_oxide::deflate::compress_to_vec_zlib(&raw, 6));
        if out.len() > limits::MAX_FILE_BYTES {
            return Err(ModelError::TooLarge(out.len(), limits::MAX_FILE_BYTES));
        }
        Ok(out)
    }

    pub fn decode(file: &[u8]) -> Result<Patch, ModelError> {
        if file.len() > limits::MAX_FILE_BYTES {
            return Err(ModelError::TooLarge(file.len(), limits::MAX_FILE_BYTES));
        }
        if file.len() < HEADER_BYTES || file[..4] != MAGIC {
            return Err(ModelError::Magic);
        }
        if u16::from_le_bytes([file[4], file[5]]) != 0 {
            return Err(ModelError::Range("flags"));
        }
        let frame = file[6];
        if file[7] != TEX_BC1_SRGB {
            return Err(ModelError::TextureFormat(file[7]));
        }
        let raw_len = u32::from_le_bytes([file[8], file[9], file[10], file[11]]) as usize;
        if !(8..=limits::MAX_RAW_BYTES).contains(&raw_len) {
            return Err(ModelError::Range("payload length"));
        }
        let raw =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&file[HEADER_BYTES..], raw_len)
                .map_err(|_| ModelError::Inflate)?;
        if raw.len() != raw_len {
            return Err(ModelError::Inflate);
        }
        let mut r = Reader { buf: &raw, at: 0 };
        let key_len = r.u8()? as usize;
        if key_len == 0 || key_len > limits::MAX_BODY_KEY {
            return Err(ModelError::Range("body key"));
        }
        let body = String::from_utf8(r.bytes(key_len)?.to_vec())
            .map_err(|_| ModelError::Range("body key"))?;
        let tex_w = r.u16()?;
        let tex_h = r.u16()?;
        let mips = r.u8()? as usize;
        r.bytes(3)?;
        if !valid_side(tex_w) || !valid_side(tex_h) {
            return Err(ModelError::Range("texture size"));
        }
        if mips != mip_count(tex_w, tex_h) as usize {
            return Err(ModelError::Range("mip count"));
        }
        let mut levels = Vec::with_capacity(mips);
        for level in 0..mips {
            let count = r.u32()? as usize;
            let (w, h) = ((tex_w >> level) as usize, (tex_h >> level) as usize);
            if count > w.div_ceil(4) * h.div_ceil(4) {
                return Err(ModelError::Range("block count"));
            }
            let mut indices = Vec::with_capacity(count);
            let mut last = 0u32;
            for n in 0..count {
                let gap = r.u32()?;
                let i = last
                    .checked_add(gap)
                    .ok_or(ModelError::Range("block index"))?;
                if n > 0 && gap == 0 {
                    return Err(ModelError::Range("block index"));
                }
                indices.push(i);
                last = i;
            }
            let blocks = r.bytes(count * 8)?.as_chunks::<8>().0;
            levels.push(indices.into_iter().zip(blocks.iter().copied()).collect());
        }
        if r.at != raw.len() {
            return Err(ModelError::Length("trailing bytes"));
        }
        let patch = Patch {
            frame,
            body,
            tex_w,
            tex_h,
            levels,
        };
        patch.validate()?;
        Ok(patch)
    }
}

/// Lay `patches` over a body's BC1 mip chain, in order (a later patch over an earlier one:
/// the legs, then the chest, so the jerkin's hem lies over the waistband): a block the cloth
/// fills whole replaces the body's, a block it fills in part is decoded, the cloth's texels
/// put over the skin's, and encoded again. Returns the blocks changed. A patch of another
/// atlas size is skipped (it was baked for another body).
pub fn composite(texture: &mut [u8], tex_w: u16, tex_h: u16, patches: &[&Patch]) -> usize {
    let mut changed = 0;
    let mut offset = 0;
    for level in 0..mip_count(tex_w, tex_h) as usize {
        let (w, h) = ((tex_w >> level) as usize, (tex_h >> level) as usize);
        let len = bc1_level_bytes(w, h);
        let Some(dst) = texture.get_mut(offset..offset + len) else {
            break;
        };
        for p in patches {
            if p.tex_w != tex_w || p.tex_h != tex_h {
                continue;
            }
            let Some(blocks) = p.levels.get(level) else {
                continue;
            };
            for (i, cloth) in blocks {
                let Some(body) = dst.get_mut(*i as usize * 8..*i as usize * 8 + 8) else {
                    continue;
                };
                if block_cut(cloth) {
                    let skin = bc1::decode_block(body);
                    let over = bc1::decode_block(cloth);
                    let mut px = [[0u8; 4]; 16];
                    for k in 0..16 {
                        px[k] = if over[k][3] != 0 { over[k] } else { skin[k] };
                    }
                    body.copy_from_slice(&encode_block(&px));
                } else {
                    body.copy_from_slice(cloth);
                }
                changed += 1;
            }
        }
        offset += len;
    }
    changed
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], ModelError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or(ModelError::Length("payload"))?;
        let s = &self.buf[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, ModelError> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ModelError> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, ModelError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::texture_bytes;

    fn flat(c: [u8; 4]) -> [[u8; 4]; 16] {
        [c; 16]
    }

    #[test]
    fn a_block_round_trips_through_the_range_fit() {
        let red = encode_block(&flat([255, 0, 0, 255]));
        assert!(!block_cut(&red) && block_has_cloth(&red));
        for p in bc1::decode_block(&red) {
            assert_eq!(p, [255, 0, 0, 255]);
        }
        // Half cloth, half nothing: the cut texels stay cut, the cloth stays red.
        let mut px = flat([255, 0, 0, 255]);
        for p in px.iter_mut().take(8) {
            *p = [0, 0, 0, 0];
        }
        let half = encode_block(&px);
        assert!(block_cut(&half) && block_has_cloth(&half));
        let back = bc1::decode_block(&half);
        assert!(back[..8].iter().all(|p| p[3] == 0));
        assert!(back[8..].iter().all(|p| p[0] > 240 && p[3] == 255));
        // Nothing at all.
        let none = encode_block(&flat([9, 9, 9, 0]));
        assert!(!block_has_cloth(&none));
        // A gradient keeps its order within BC1's 5:6:5.
        let mut px = [[0u8; 4]; 16];
        for (i, p) in px.iter_mut().enumerate() {
            *p = [(i * 17) as u8, 40, 200, 255];
        }
        let back = bc1::decode_block(&encode_block(&px));
        assert!(back[0][0] < 40 && back[15][0] > 220);
        assert!(
            back.windows(2)
                .all(|w| w[0][0] as i32 <= w[1][0] as i32 + 1)
        );
    }

    fn sample_patch() -> Patch {
        let (w, h) = (64u16, 64u16);
        // A cutout chain: one red block at the top left of each level, the rest cut.
        let mut chain = vec![0u8; texture_bytes(w, h)];
        let mut offset = 0;
        for level in 0..mip_count(w, h) {
            let (lw, lh) = ((w >> level) as usize, (h >> level) as usize);
            let len = bc1_level_bytes(lw, lh);
            for b in chain[offset..offset + len].as_chunks_mut::<8>().0 {
                *b = [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
            }
            chain[offset..offset + 8].copy_from_slice(&encode_block(&flat([255, 0, 0, 255])));
            // And a half block beside it on the first level.
            if level == 0 {
                let mut px = flat([0, 255, 0, 255]);
                px[0] = [0, 0, 0, 0];
                chain[offset + 8..offset + 16].copy_from_slice(&encode_block(&px));
            }
            offset += len;
        }
        Patch::from_chain(1, "striker_mail", w, h, &chain)
    }

    #[test]
    fn a_patch_keeps_only_the_blocks_with_cloth_and_round_trips() {
        let p = sample_patch();
        assert_eq!(p.levels.len(), mip_count(64, 64) as usize);
        assert_eq!(p.levels[0].len(), 2);
        assert!(p.levels[1..].iter().all(|l| l.len() == 1));
        assert_eq!(p.levels[0][0].0, 0);
        assert_eq!(p.levels[0][1].0, 1);
        let bytes = p.encode().unwrap();
        assert_eq!(&bytes[..4], b"GMP1");
        assert_eq!(Patch::decode(&bytes).unwrap(), p);
        // The bytes are pinned: a change of the format is a change here.
        assert_eq!(bytes.len(), 73, "{bytes:?}");
        assert_eq!(Patch::decode(b"junk"), Err(ModelError::Magic));
        let mut bad = p.clone();
        bad.levels[0][1].0 = 0;
        assert_eq!(bad.validate(), Err(ModelError::Range("block index")));
        let mut other = p.clone();
        other.body = "Bad Key".into();
        assert_eq!(other.validate(), Err(ModelError::Range("body key")));
        let mut short = p.clone();
        short.levels.pop();
        assert_eq!(short.validate(), Err(ModelError::Range("mip count")));
    }

    #[test]
    fn the_composite_lays_cloth_over_skin_block_by_block() {
        let p = sample_patch();
        // A blue body.
        let mut body = Vec::new();
        for level in 0..mip_count(64, 64) {
            let (lw, lh) = ((64 >> level) as usize, (64 >> level) as usize);
            for _ in 0..bc1_level_bytes(lw, lh) / 8 {
                body.extend_from_slice(&encode_block(&flat([0, 0, 255, 255])));
            }
        }
        let before = body.clone();
        let changed = composite(&mut body, 64, 64, &[&p]);
        assert_eq!(changed, p.blocks());
        // Block 0 is red whole, block 1 green but its first texel, block 2 untouched.
        let b0 = bc1::decode_block(&body[..8]);
        assert!(b0.iter().all(|t| t[0] > 240 && t[3] == 255));
        let b1 = bc1::decode_block(&body[8..16]);
        assert!(
            b1[0][2] > 240 && b1[0][3] == 255,
            "the skin shows: {:?}",
            b1[0]
        );
        assert!(b1[1..].iter().all(|t| t[1] > 240));
        assert_eq!(&body[16..24], &before[16..24]);
        // Another size is another body: nothing changes.
        let mut small = vec![0u8; texture_bytes(64, 32)];
        assert_eq!(composite(&mut small, 64, 32, &[&p]), 0);
    }
}
