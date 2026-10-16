//! ETC2 RGB8 block coding in plain Rust (`COMPRESSED_RGB8_ETC2`): the encoder for the GPUs
//! that have ETC2 and not BC1 (mobile, MODELS.md 8), and the decoder for a client without
//! either, previews and tests. A block is 4×4 texels in 8 bytes, one 64-bit word stored
//! big-endian as the Khronos spec draws it, bit 63 first. Texel `(x, y)` of a block has index
//! `x * 4 + y`, so the sixteen two-bit selectors run down the columns.
//!
//! The encoder writes ETC1's individual and differential modes with all eight modifier tables
//! and both flips, choosing the base colours by a search around each half's mean, and ETC2's
//! T and H modes for the blocks ETC1 fits badly (two clusters of colour, an edge). Planar mode
//! is decoded but never written. Punch-through alpha and the EAC alpha plane are out of scope:
//! this is RGB8, and the alpha of the input is ignored. Everything is integer arithmetic, so
//! the same image encodes to the same bytes on every machine.
//!
//! Standalone for now: wiring into the `.gmm` container, the ingestion and the client comes
//! with the texture format change that follows.

/// One block: 4×4 texels in 64 bits.
pub const BLOCK_BYTES: usize = 8;

/// ETC1's intensity modifier tables, `[small, large]` per codeword: a texel is its half's base
/// colour plus or minus one of the two (spec table C.8).
const MODIFIERS: [[i32; 2]; 8] = [
    [2, 8],
    [5, 17],
    [9, 29],
    [13, 42],
    [18, 60],
    [24, 80],
    [33, 106],
    [47, 183],
];

/// The distances of the T and H modes (spec table C.11).
const DISTANCES: [i32; 8] = [3, 6, 11, 16, 23, 32, 41, 64];

/// How bad an ETC1 fit (the sum of squared channel errors over the block) has to be before
/// the T and H modes are tried: a root-mean-square error of six per channel. Below that the
/// block is smooth or merely noisy, and the two modes do not win.
const TH_GATE: i32 = 16 * 3 * 6 * 6;

/// The modifier a two-bit selector picks from a table: `00` is `+small`, `01` `+large`, `10`
/// `−small` and `11` `−large` (spec table C.9).
fn modifier(table: usize, selector: usize) -> i32 {
    let [small, large] = MODIFIERS[table];
    match selector {
        0 => small,
        1 => large,
        2 => -small,
        _ => -large,
    }
}

/// Bits `hi` down to `hi + 1 − n` of a block word, numbered as the spec numbers them.
fn field(word: u64, hi: u32, n: u32) -> i32 {
    ((word >> (hi + 1 - n)) & ((1u64 << n) - 1)) as i32
}

/// Store the low `n` bits of `v` at bits `hi` down to `hi + 1 − n`.
fn put(word: &mut u64, hi: u32, n: u32, v: i32) {
    *word |= ((v as u64) & ((1u64 << n) - 1)) << (hi + 1 - n);
}

/// A three-bit two's-complement field.
fn sign3(v: i32) -> i32 {
    if v >= 4 { v - 8 } else { v }
}

/// Bit replication from `bits` to eight, how every mode widens its stored colours.
fn expand(q: i32, bits: u32) -> i32 {
    match bits {
        4 => q * 17,
        5 => (q << 3) | (q >> 2),
        6 => (q << 2) | (q >> 4),
        _ => (q << 1) | (q >> 6),
    }
}

/// The `bits`-wide value whose expansion is nearest an eight-bit one.
fn quantize(v: i32, bits: u32) -> i32 {
    let max = (1 << bits) - 1;
    ((v * max + 127) / 255).clamp(0, max)
}

fn expand3(q: [i32; 3], bits: u32) -> [i32; 3] {
    q.map(|c| expand(c, bits))
}

fn offset(c: [i32; 3], d: i32) -> [i32; 3] {
    c.map(|v| (v + d).clamp(0, 255))
}

fn dist2(a: [i32; 3], b: [i32; 3]) -> i32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// The two-bit selector of texel `i`: its high bit is bit `16 + i`, its low bit `i`.
fn selector(word: u64, i: usize) -> usize {
    ((((word >> (16 + i)) & 1) << 1) | ((word >> i) & 1)) as usize
}

/// Texel `k` of half `s`: the halves are the left and right 2×4 columns, or with the flip the
/// top and bottom 4×2 rows, each in the column-major order of the selector bits.
fn texel_of(flip: bool, s: usize, k: usize) -> usize {
    if flip {
        (k / 2) * 4 + 2 * s + k % 2
    } else {
        8 * s + k
    }
}

/// Decode one block to its sixteen texels in column-major order.
fn decode_block(word: u64) -> [[i32; 3]; 16] {
    let diff = field(word, 33, 1) == 1;
    if diff {
        let sum = |hi: u32| field(word, hi, 5) + sign3(field(word, hi - 5, 3));
        let (r, g, b) = (sum(63), sum(55), sum(47));
        let overflows = |c: i32| !(0..=31).contains(&c);
        if overflows(r) {
            return paint(word, t_paints(word));
        }
        if overflows(g) {
            return paint(word, h_paints(word));
        }
        if overflows(b) {
            return planar(word);
        }
    }
    let (base0, base1) = if diff {
        let q = [field(word, 63, 5), field(word, 55, 5), field(word, 47, 5)];
        let d = [field(word, 58, 3), field(word, 50, 3), field(word, 42, 3)].map(sign3);
        let q1 = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
        (expand3(q, 5), expand3(q1, 5))
    } else {
        let q0 = [field(word, 63, 4), field(word, 55, 4), field(word, 47, 4)];
        let q1 = [field(word, 59, 4), field(word, 51, 4), field(word, 43, 4)];
        (expand3(q0, 4), expand3(q1, 4))
    };
    let tables = [field(word, 39, 3) as usize, field(word, 36, 3) as usize];
    let flip = field(word, 32, 1) == 1;
    let mut out = [[0; 3]; 16];
    for (i, texel) in out.iter_mut().enumerate() {
        let half = if flip { (i % 4) / 2 } else { i / 8 };
        let base = if half == 0 { base0 } else { base1 };
        *texel = offset(base, modifier(tables[half], selector(word, i)));
    }
    out
}

/// Every texel takes one of four paint colours by its selector (T and H modes).
fn paint(word: u64, paints: [[i32; 3]; 4]) -> [[i32; 3]; 16] {
    let mut out = [[0; 3]; 16];
    for (i, texel) in out.iter_mut().enumerate() {
        *texel = paints[selector(word, i)];
    }
    out
}

/// T mode: a flat colour and a second one spread by a distance.
fn t_paints(word: u64) -> [[i32; 3]; 4] {
    let c0 = expand3(
        [
            (field(word, 60, 2) << 2) | field(word, 57, 2),
            field(word, 55, 4),
            field(word, 51, 4),
        ],
        4,
    );
    let c1 = expand3(
        [field(word, 47, 4), field(word, 43, 4), field(word, 39, 4)],
        4,
    );
    let d = DISTANCES[((field(word, 35, 2) << 1) | field(word, 32, 1)) as usize];
    [c0, offset(c1, d), c1, offset(c1, -d)]
}

/// H mode: two colours, each spread by the one distance, whose low bit is the order of the
/// two colours as twelve-bit numbers.
fn h_paints(word: u64) -> [[i32; 3]; 4] {
    let q0 = [
        field(word, 62, 4),
        (field(word, 58, 3) << 1) | field(word, 52, 1),
        (field(word, 51, 1) << 3) | field(word, 49, 3),
    ];
    let q1 = [field(word, 46, 4), field(word, 42, 4), field(word, 38, 4)];
    let low = i32::from(value12(q0) >= value12(q1));
    let d = DISTANCES[((field(word, 34, 1) << 2) | (field(word, 32, 1) << 1) | low) as usize];
    let (c0, c1) = (expand3(q0, 4), expand3(q1, 4));
    [offset(c0, d), offset(c0, -d), offset(c1, d), offset(c1, -d)]
}

fn value12(q: [i32; 3]) -> i32 {
    (q[0] << 8) | (q[1] << 4) | q[2]
}

/// Planar mode: a colour at the origin and the colours one block to the right and below,
/// interpolated.
fn planar(word: u64) -> [[i32; 3]; 16] {
    let o = [
        expand(field(word, 62, 6), 6),
        expand((field(word, 56, 1) << 6) | field(word, 54, 6), 7),
        expand(
            (field(word, 48, 1) << 5) | (field(word, 44, 2) << 3) | field(word, 41, 3),
            6,
        ),
    ];
    let h = [
        expand((field(word, 38, 5) << 1) | field(word, 32, 1), 6),
        expand(field(word, 31, 7), 7),
        expand(field(word, 24, 6), 6),
    ];
    let v = [
        expand(field(word, 18, 6), 6),
        expand(field(word, 12, 7), 7),
        expand(field(word, 5, 6), 6),
    ];
    let mut out = [[0; 3]; 16];
    for (i, texel) in out.iter_mut().enumerate() {
        let (x, y) = ((i / 4) as i32, (i % 4) as i32);
        for (c, t) in texel.iter_mut().enumerate() {
            *t = ((x * (h[c] - o[c]) + y * (v[c] - o[c]) + 4 * o[c] + 2) >> 2).clamp(0, 255);
        }
    }
    out
}

/// Decode one block to RGBA8 texels in row-major order (`y * 4 + x`, as `bc1::decode_block`
/// gives them), alpha 255.
pub fn decode_block_rgba(block: &[u8; 8]) -> [[u8; 4]; 16] {
    let texels = decode_block(u64::from_be_bytes(*block));
    std::array::from_fn(|i| {
        let (x, y) = (i % 4, i / 4);
        let t = texels[x * 4 + y];
        [t[0] as u8, t[1] as u8, t[2] as u8, 255]
    })
}

/// Encode sixteen RGBA8 texels in row-major order as one block; alpha is ignored.
pub fn encode_block_rgba(px: &[[u8; 4]; 16]) -> [u8; 8] {
    encode_block(&column_major(px)).to_be_bytes()
}

/// The same with ETC1's modes alone (no T or H): what the client's composite uses, a few
/// times faster on the blocks the cloth touches, at the quality ETC1 gives them.
pub fn encode_block_rgba_fast(px: &[[u8; 4]; 16]) -> [u8; 8] {
    encode_etc1_with(&column_major(px), true).0.to_be_bytes()
}

fn column_major(px: &[[u8; 4]; 16]) -> [[i32; 3]; 16] {
    let mut cm = [[0i32; 3]; 16];
    for (k, t) in cm.iter_mut().enumerate() {
        let (x, y) = (k / 4, k % 4);
        let p = px[y * 4 + x];
        *t = [i32::from(p[0]), i32::from(p[1]), i32::from(p[2])];
    }
    cm
}

/// Decode one level of ETC2 RGB8 blocks to RGBA8 with alpha 255. `blocks` must hold
/// `ceil(w / 4) × ceil(h / 4) × 8` bytes; a short buffer decodes as far as it goes.
pub fn decode(blocks: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let bw = w.div_ceil(4);
    for (i, block) in blocks.as_chunks::<BLOCK_BYTES>().0.iter().enumerate() {
        let (bx, by) = (i % bw, i / bw);
        if by * 4 >= h {
            break;
        }
        let texels = decode_block(u64::from_be_bytes(*block));
        for (k, texel) in texels.iter().enumerate() {
            let (x, y) = (bx * 4 + k / 4, by * 4 + k % 4);
            if x < w && y < h {
                let at = (y * w + x) * 4;
                out[at..at + 4].copy_from_slice(&[
                    texel[0] as u8,
                    texel[1] as u8,
                    texel[2] as u8,
                    255,
                ]);
            }
        }
    }
    out
}

/// Encode an RGBA8 image, rows top-down, to ETC2 RGB8 blocks: `ceil(w / 4) × ceil(h / 4) × 8`
/// bytes, rows of blocks top-down. A side that is not a multiple of four is padded by
/// replicating the last column or row. Alpha is ignored.
pub fn encode(rgba: &[u8], w: usize, h: usize) -> Vec<u8> {
    assert!(
        rgba.len() >= w * h * 4,
        "etc::encode: {w}×{h} needs {} bytes",
        w * h * 4
    );
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = Vec::with_capacity(bw * bh * BLOCK_BYTES);
    if w == 0 || h == 0 {
        return out;
    }
    let mut px = [[0i32; 3]; 16];
    for by in 0..bh {
        for bx in 0..bw {
            for (i, p) in px.iter_mut().enumerate() {
                let x = (bx * 4 + i / 4).min(w - 1);
                let y = (by * 4 + i % 4).min(h - 1);
                let at = (y * w + x) * 4;
                *p = [
                    i32::from(rgba[at]),
                    i32::from(rgba[at + 1]),
                    i32::from(rgba[at + 2]),
                ];
            }
            out.extend_from_slice(&encode_block(&px).to_be_bytes());
        }
    }
    out
}

/// The best block for sixteen texels in column-major order.
fn encode_block(px: &[[i32; 3]; 16]) -> u64 {
    let (word, err) = encode_etc1(px);
    if err <= TH_GATE {
        return word;
    }
    match encode_th(px, err) {
        Some((better, _)) => better,
        None => word,
    }
}

/// One half's base colour, table and selectors, with the sum of squared errors they leave.
#[derive(Clone, Copy)]
struct Fit {
    q: [i32; 3],
    table: usize,
    sel: [u8; 8],
    err: i32,
}

/// The selectors and error of one base colour and table over a half, or `None` once the error
/// reaches `limit`. The selector is chosen by where the texel projects onto the grey axis the
/// four candidates lie on, which is the nearest of them short of clamping.
fn fit_table(
    tex: &[[i32; 3]; 8],
    base: [i32; 3],
    table: usize,
    limit: i32,
) -> Option<([u8; 8], i32)> {
    let [small, large] = MODIFIERS[table];
    let threshold = 3 * (small + large);
    let mut err = 0;
    let mut sel = [0u8; 8];
    for (k, p) in tex.iter().enumerate() {
        let t = (p[0] - base[0]) + (p[1] - base[1]) + (p[2] - base[2]);
        let (s, m) = if t >= 0 {
            if 2 * t > threshold {
                (1, large)
            } else {
                (0, small)
            }
        } else if -2 * t > threshold {
            (3, -large)
        } else {
            (2, -small)
        };
        for (b, p) in base.iter().zip(p) {
            let d = (b + m).clamp(0, 255) - p;
            err += d * d;
        }
        if err >= limit {
            return None;
        }
        sel[k] = s;
    }
    Some((sel, err))
}

/// The best fit of a `bits`-wide base colour within `[lo, hi]` to a half, if one has an error
/// below `limit`: every table at the quantized mean, then the mean's neighbours at the best
/// table and the two beside it. The first pass runs in full, so that the tables of the
/// second are chosen well even when the mean itself is over the limit.
fn fit_half(
    tex: &[[i32; 3]; 8],
    bits: u32,
    lo: [i32; 3],
    hi: [i32; 3],
    limit: i32,
    quick: bool,
) -> Option<Fit> {
    let mut sum = [0i32; 3];
    for p in tex {
        for (s, c) in sum.iter_mut().zip(p) {
            *s += c;
        }
    }
    let centre: [i32; 3] =
        std::array::from_fn(|c| quantize((sum[c] + 4) / 8, bits).clamp(lo[c], hi[c]));
    let mut best: Option<Fit> = None;
    let mut bound = i32::MAX;
    let base = expand3(centre, bits);
    for table in 0..8 {
        if let Some((sel, err)) = fit_table(tex, base, table, bound) {
            best = Some(Fit {
                q: centre,
                table,
                sel,
                err,
            });
            bound = err;
        }
    }
    let found = best?;
    if found.err == 0 || quick {
        return best.filter(|f| f.err < limit);
    }
    let tables = found.table.saturating_sub(1)..=(found.table + 1).min(7);
    bound = bound.min(limit);
    for step in 1..27 {
        let q = [
            centre[0] + step % 3 - 1,
            centre[1] + (step / 3) % 3 - 1,
            centre[2] + step / 9 - 1,
        ];
        if step == 13 || (0..3).any(|c| q[c] < lo[c] || q[c] > hi[c]) {
            continue;
        }
        let base = expand3(q, bits);
        for table in tables.clone() {
            if let Some((sel, err)) = fit_table(tex, base, table, bound) {
                best = Some(Fit { q, table, sel, err });
                bound = err;
            }
        }
    }
    best.filter(|f| f.err < limit)
}

/// The range a differential-mode half may take next to the other half's `anchor`: the stored
/// difference is the second half's less the first's, in `−4..=3`.
fn diff_range(anchor: [i32; 3], second: bool) -> ([i32; 3], [i32; 3]) {
    if second {
        (
            anchor.map(|c| (c - 4).max(0)),
            anchor.map(|c| (c + 3).min(31)),
        )
    } else {
        (
            anchor.map(|c| (c - 3).max(0)),
            anchor.map(|c| (c + 4).min(31)),
        )
    }
}

/// The best ETC1 block (individual or differential, either flip) and its error. Each half is
/// searched under the error the block as a whole still has to beat, so later candidates
/// bail out early.
fn encode_etc1(px: &[[i32; 3]; 16]) -> (u64, i32) {
    encode_etc1_with(px, false)
}

/// The same, and with `quick` the mean alone as each half's base (no neighbours): the
/// client's composite, a few times faster.
fn encode_etc1_with(px: &[[i32; 3]; 16], quick: bool) -> (u64, i32) {
    let mut best = (0u64, i32::MAX);
    for flip in [false, true] {
        let halves: [[[i32; 3]; 8]; 2] =
            std::array::from_fn(|s| std::array::from_fn(|k| px[texel_of(flip, s, k)]));
        if let Some(f0) = fit_half(&halves[0], 4, [0; 3], [15; 3], best.1) {
            let f1 = fit_half(&halves[1], 4, [0; 3], [15; 3], best.1 - f0.err);
            consider(&mut best, flip, false, f0, f1);
        }
        let Some(f0) = fit_half(&halves[0], 5, [0; 3], [31; 3], best.1) else {
            continue;
        };
        let Some(f1) = fit_half(&halves[1], 5, [0; 3], [31; 3], best.1 - f0.err) else {
            continue;
        };
        if (0..3).all(|c| (-4..=3).contains(&(f1.q[c] - f0.q[c]))) {
            consider(&mut best, flip, true, f0, Some(f1));
        } else {
            let (lo, hi) = diff_range(f0.q, true);
            let second = fit_half(&halves[1], 5, lo, hi, best.1 - f0.err, quick);
            consider(&mut best, flip, true, f0, second);
            let (lo, hi) = diff_range(f1.q, false);
            if let Some(first) = fit_half(&halves[0], 5, lo, hi, best.1 - f1.err, quick) {
                consider(&mut best, flip, true, first, Some(f1));
            }
        }
    }
    best
}

/// Take a pair of half fits as the block's best: the second was searched under what the
/// block had left to beat, so its presence means the pair wins.
fn consider(best: &mut (u64, i32), flip: bool, diff: bool, f0: Fit, f1: Option<Fit>) {
    if let Some(f1) = f1 {
        *best = (pack_etc1(flip, diff, &[f0, f1]), f0.err + f1.err);
    }
}

fn pack_etc1(flip: bool, diff: bool, fits: &[Fit; 2]) -> u64 {
    let mut word = 0u64;
    let (q0, q1) = (fits[0].q, fits[1].q);
    for (c, hi) in [63u32, 55, 47].into_iter().enumerate() {
        if diff {
            put(&mut word, hi, 5, q0[c]);
            put(&mut word, hi - 5, 3, q1[c] - q0[c]);
        } else {
            put(&mut word, hi, 4, q0[c]);
            put(&mut word, hi - 4, 4, q1[c]);
        }
    }
    put(&mut word, 39, 3, fits[0].table as i32);
    put(&mut word, 36, 3, fits[1].table as i32);
    put(&mut word, 33, 1, i32::from(diff));
    put(&mut word, 32, 1, i32::from(flip));
    for (s, fit) in fits.iter().enumerate() {
        for (k, sel) in fit.sel.iter().enumerate() {
            put_selector(&mut word, texel_of(flip, s, k), *sel);
        }
    }
    word
}

fn put_selector(word: &mut u64, i: usize, sel: u8) {
    *word |= u64::from(sel >> 1) << (16 + i);
    *word |= u64::from(sel & 1) << i;
}

/// The selectors and error of four paint colours over the whole block, or `None` once the
/// error reaches `limit`.
fn fit_paints(px: &[[i32; 3]; 16], paints: &[[i32; 3]; 4], limit: i32) -> Option<([u8; 16], i32)> {
    let mut err = 0;
    let mut sel = [0u8; 16];
    for (i, p) in px.iter().enumerate() {
        let mut best = (i32::MAX, 0u8);
        for (j, c) in paints.iter().enumerate() {
            let e = dist2(*c, *p);
            if e < best.0 {
                best = (e, j as u8);
            }
        }
        err += best.0;
        if err >= limit {
            return None;
        }
        sel[i] = best.1;
    }
    Some((sel, err))
}

/// Two colour clusters of the block, as four-bit colours: seeded by the two texels farthest
/// apart, then a few rounds of means.
fn clusters(px: &[[i32; 3]; 16]) -> ([i32; 3], [i32; 3]) {
    let (mut a, mut b, mut far) = (0, 0, -1);
    for (i, p) in px.iter().enumerate() {
        for (j, q) in px.iter().enumerate().skip(i + 1) {
            let d = dist2(*p, *q);
            if d > far {
                (a, b, far) = (i, j, d);
            }
        }
    }
    let mut means = [px[a], px[b]];
    for _ in 0..4 {
        let mut sums = [[0i32; 4]; 2];
        for p in px {
            let near = usize::from(dist2(*p, means[1]) < dist2(*p, means[0]));
            for (s, c) in sums[near].iter_mut().zip(p) {
                *s += c;
            }
            sums[near][3] += 1;
        }
        for (mean, sum) in means.iter_mut().zip(&sums) {
            if sum[3] > 0 {
                *mean = [sum[0], sum[1], sum[2]].map(|s| (s + sum[3] / 2) / sum[3]);
            }
        }
    }
    (
        means[0].map(|c| quantize(c, 4)),
        means[1].map(|c| quantize(c, 4)),
    )
}

/// The best T or H block for the clusters of the block, if one beats `limit`.
fn encode_th(px: &[[i32; 3]; 16], limit: i32) -> Option<(u64, i32)> {
    let (ca, cb) = clusters(px);
    let mut best: Option<(u64, i32)> = None;
    let mut limit = limit;
    for (c0, c1) in [(ca, cb), (cb, ca)] {
        let (e0, e1) = (expand3(c0, 4), expand3(c1, 4));
        for (d, &dist) in DISTANCES.iter().enumerate() {
            let paints = [e0, offset(e1, dist), e1, offset(e1, -dist)];
            if let Some((sel, err)) = fit_paints(px, &paints, limit) {
                best = Some((pack_t(c0, c1, d as i32, &sel), err));
                limit = err;
            }
        }
    }
    for (d, &dist) in DISTANCES.iter().enumerate() {
        let (mut c0, mut c1) = (ca, cb);
        let (v0, v1) = (value12(c0), value12(c1));
        let low = (d & 1) == 1;
        if v0 == v1 {
            if !low {
                continue;
            }
        } else if (v0 >= v1) != low {
            std::mem::swap(&mut c0, &mut c1);
        }
        let (e0, e1) = (expand3(c0, 4), expand3(c1, 4));
        let paints = [
            offset(e0, dist),
            offset(e0, -dist),
            offset(e1, dist),
            offset(e1, -dist),
        ];
        if let Some((sel, err)) = fit_paints(px, &paints, limit) {
            best = Some((pack_h(c0, c1, d as i32, &sel), err));
            limit = err;
        }
    }
    best
}

/// A T-mode block. The spare bits are set so that red overflows the differential decode,
/// which is what selects the mode.
fn pack_t(c0: [i32; 3], c1: [i32; 3], d: i32, sel: &[u8; 16]) -> u64 {
    let mut word = 0u64;
    let (r0a, r0b) = (c0[0] >> 2, c0[0] & 3);
    put(&mut word, 60, 2, r0a);
    put(&mut word, 57, 2, r0b);
    put(&mut word, 55, 4, c0[1]);
    put(&mut word, 51, 4, c0[2]);
    put(&mut word, 47, 4, c1[0]);
    put(&mut word, 43, 4, c1[1]);
    put(&mut word, 39, 4, c1[2]);
    put(&mut word, 35, 2, d >> 1);
    put(&mut word, 33, 1, 1);
    put(&mut word, 32, 1, d & 1);
    if r0a + r0b < 4 {
        put(&mut word, 58, 1, 1);
    } else {
        put(&mut word, 63, 3, 7);
    }
    for (i, s) in sel.iter().enumerate() {
        put_selector(&mut word, i, *s);
    }
    word
}

/// An H-mode block: the spare bits keep red in range and push green out of it. The low bit of
/// the distance is implied by the order of the two colours, which the caller has arranged.
fn pack_h(c0: [i32; 3], c1: [i32; 3], d: i32, sel: &[u8; 16]) -> u64 {
    let mut word = 0u64;
    put(&mut word, 62, 4, c0[0]);
    put(&mut word, 58, 3, c0[1] >> 1);
    put(&mut word, 52, 1, c0[1] & 1);
    put(&mut word, 51, 1, c0[2] >> 3);
    put(&mut word, 49, 3, c0[2] & 7);
    put(&mut word, 46, 4, c1[0]);
    put(&mut word, 42, 4, c1[1]);
    put(&mut word, 38, 4, c1[2]);
    put(&mut word, 34, 1, d >> 2);
    put(&mut word, 33, 1, 1);
    put(&mut word, 32, 1, (d >> 1) & 1);
    if c0[0] + sign3(c0[1] >> 1) < 0 {
        put(&mut word, 63, 1, 1);
    }
    let (g, dg) = (((c0[1] & 1) << 1) | (c0[2] >> 3), (c0[2] >> 1) & 3);
    if g + dg < 4 {
        put(&mut word, 50, 1, 1);
    } else {
        put(&mut word, 55, 3, 7);
    }
    for (i, s) in sel.iter().enumerate() {
        put_selector(&mut word, i, *s);
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peak signal-to-noise ratio over the RGB channels of two RGBA images, in decibels.
    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let (mut sum, mut n) = (0f64, 0f64);
        for (pa, pb) in a.chunks(4).zip(b.chunks(4)) {
            for c in 0..3 {
                let d = f64::from(pa[c]) - f64::from(pb[c]);
                sum += d * d;
                n += 1.0;
            }
        }
        if sum == 0.0 {
            return f64::INFINITY;
        }
        10.0 * (255.0f64 * 255.0 / (sum / n)).log10()
    }

    /// The mode a block decodes in, by the overflow rule of the spec.
    fn mode(word: u64) -> &'static str {
        if field(word, 33, 1) == 0 {
            return "individual";
        }
        let sum = |hi: u32| field(word, hi, 5) + sign3(field(word, hi - 5, 3));
        let over = |c: i32| !(0..=31).contains(&c);
        if over(sum(63)) {
            "T"
        } else if over(sum(55)) {
            "H"
        } else if over(sum(47)) {
            "planar"
        } else {
            "differential"
        }
    }

    fn flat(w: usize, h: usize, c: [u8; 3]) -> Vec<u8> {
        [c[0], c[1], c[2], 255].repeat(w * h)
    }

    fn gradient(w: usize, h: usize) -> Vec<u8> {
        let mut img = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let r = (x * 255 / (w - 1)) as u8;
                let g = (y * 255 / (h - 1)) as u8;
                let b = ((x + y) * 255 / (w + h - 2)) as u8;
                img.extend_from_slice(&[r, g, b, 255]);
            }
        }
        img
    }

    /// The gradient with texel noise over it: a luminance jitter of ±24 shared by the three
    /// channels and a chroma jitter of ±6 per channel, from a fixed xorshift stream.
    fn noisy(w: usize, h: usize) -> Vec<u8> {
        let mut img = gradient(w, h);
        let mut state = 0x9E37_79B9u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for px in img.chunks_mut(4) {
            let luma = (next() % 49) as i32 - 24;
            for c in px.iter_mut().take(3) {
                let chroma = (next() % 13) as i32 - 6;
                *c = (i32::from(*c) + luma + chroma).clamp(0, 255) as u8;
            }
        }
        img
    }

    fn round_trip(img: &[u8], w: usize, h: usize) -> Vec<u8> {
        let blocks = encode(img, w, h);
        assert_eq!(blocks.len(), w.div_ceil(4) * h.div_ceil(4) * BLOCK_BYTES);
        decode(&blocks, w, h)
    }

    #[test]
    fn hand_assembled_etc1_blocks_decode() {
        // Individual mode: half 0 is red (15, 0, 0) with table 0, half 1 is blue (0, 0, 15)
        // with table 2, no flip. Texels 0..4 take selectors 0..4; texel 8 takes selector 1.
        let block = [0xF0, 0x00, 0x0F, 0x08, 0x00, 0x0C, 0x01, 0x0A];
        let px = decode(&block, 4, 4);
        let at = |x: usize, y: usize| &px[(y * 4 + x) * 4..][..4];
        assert_eq!(at(0, 0), [255, 2, 2, 255]);
        assert_eq!(at(0, 1), [255, 8, 8, 255]);
        assert_eq!(at(0, 2), [253, 0, 0, 255]);
        assert_eq!(at(0, 3), [247, 0, 0, 255]);
        assert_eq!(at(2, 0), [29, 29, 255, 255]);
        assert_eq!(at(2, 1), [9, 9, 255, 255]);
        // Differential mode with the flip: red 16 (132) and a difference of −1 (123), table 0,
        // every selector 0 (+2). The top rows are half 0, the bottom rows half 1.
        let block = [0x87, 0x00, 0x00, 0x03, 0, 0, 0, 0];
        let px = decode(&block, 4, 4);
        assert_eq!(&px[0..4], [134, 2, 2, 255]);
        assert_eq!(&px[(2 * 4) * 4..][..4], [125, 2, 2, 255]);
        assert_eq!(decode(&[], 8, 8).len(), 256);
    }

    #[test]
    fn hand_assembled_t_and_h_blocks_decode() {
        // T mode: flat red (15, 0, 0), blue (0, 0, 15) spread by distance index 3 (16).
        let t = [0xFB, 0x00, 0x00, 0xF7, 0x00, 0x0C, 0x00, 0x0A];
        let px = decode(&t, 4, 4);
        let at = |x: usize, y: usize| &px[(y * 4 + x) * 4..][..4];
        assert_eq!(at(0, 0), [255, 0, 0, 255]);
        assert_eq!(at(0, 1), [16, 16, 255, 255]);
        assert_eq!(at(0, 2), [0, 0, 255, 255]);
        assert_eq!(at(0, 3), [0, 0, 239, 255]);
        // H mode: the same two colours, both spread by 16; red orders above blue, which
        // supplies the distance's low bit.
        let h = [0x78, 0x04, 0x00, 0x7B, 0x00, 0x0C, 0x00, 0x0A];
        let px = decode(&h, 4, 4);
        let at = |x: usize, y: usize| &px[(y * 4 + x) * 4..][..4];
        assert_eq!(at(0, 0), [255, 16, 16, 255]);
        assert_eq!(at(0, 1), [239, 0, 0, 255]);
        assert_eq!(at(0, 2), [16, 16, 255, 255]);
        assert_eq!(at(0, 3), [0, 0, 239, 255]);
    }

    #[test]
    fn packed_t_and_h_blocks_select_their_modes() {
        // Every four-bit colour must trigger the right overflow whatever the spare bits had
        // to be: T through red, H through green with red kept in range.
        let sel = [0u8; 16];
        for r in 0..16 {
            for g in 0..16 {
                for b in 0..16 {
                    let c = [r, g, b];
                    let t = pack_t(c, [15 - r, 15 - g, 15 - b], 2, &sel);
                    assert_eq!(mode(t), "T", "{c:?}");
                    assert_eq!(decode_block(t)[0], expand3(c, 4), "T {c:?}");
                    let other = [r ^ 1, g, b];
                    let d = 4 | i32::from(value12(c) >= value12(other));
                    let h = pack_h(c, other, d, &sel);
                    assert_eq!(mode(h), "H", "{c:?}");
                    let expect = offset(expand3(c, 4), DISTANCES[d as usize]);
                    assert_eq!(decode_block(h)[0], expect, "H {c:?}");
                }
            }
        }
    }

    #[test]
    fn flat_colours_round_trip() {
        // Colours a base plus one modifier can reach exactly, and the clamped extremes.
        for c in [
            [0, 0, 0],
            [255, 255, 255],
            [19, 36, 53],
            [130, 130, 130],
            [238, 17, 17],
        ] {
            let img = flat(8, 8, c);
            assert_eq!(round_trip(&img, 8, 8), img, "{c:?}");
        }
        // Any flat colour comes back within a four-bit quantization step.
        for c in [[17, 34, 51], [100, 50, 200], [1, 254, 128], [77, 77, 78]] {
            let img = flat(4, 4, c);
            let out = round_trip(&img, 4, 4);
            for (a, b) in img.iter().zip(&out) {
                assert!(a.abs_diff(*b) <= 8, "{c:?}: {img:?} vs {out:?}");
            }
        }
    }

    #[test]
    fn two_colour_blocks_use_t_or_h_mode() {
        // Red and blue in a 2×2 checker: neither flip puts one colour per half, so ETC1
        // fits a purple with grey steps and the T or H mode wins, both colours exact.
        let mut img = flat(4, 4, [255, 0, 0]);
        for y in 0..4 {
            for x in 0..4 {
                if (x / 2 + y / 2) % 2 == 1 {
                    img[(y * 4 + x) * 4..][..3].copy_from_slice(&[0, 0, 255]);
                }
            }
        }
        let mut px = [[0; 3]; 16];
        for (i, p) in px.iter_mut().enumerate() {
            let at = ((i % 4) * 4 + i / 4) * 4;
            *p = [
                i32::from(img[at]),
                i32::from(img[at + 1]),
                i32::from(img[at + 2]),
            ];
        }
        let (_, etc1_err) = encode_etc1(&px);
        assert!(etc1_err > TH_GATE);
        let word = encode_block(&px);
        assert!(matches!(mode(word), "T" | "H"), "{word:#018x}");
        assert_eq!(round_trip(&img, 4, 4), img);
    }

    #[test]
    fn gradient_and_noise_reach_thirty_decibels() {
        let (w, h) = (256, 256);
        for (name, img) in [("gradient", gradient(w, h)), ("noise", noisy(w, h))] {
            let db = psnr(&img, &round_trip(&img, w, h));
            println!("etc2 {name} {w}×{h}: {db:.2} dB");
            assert!(db >= 30.0, "{name}: {db:.2} dB");
        }
    }

    #[test]
    fn odd_sizes_pad_by_replication() {
        for (w, h) in [(1, 1), (5, 3), (7, 9), (13, 4), (4, 13)] {
            let img = gradient(w.max(2), h.max(2));
            let img = &img[..w * h * 4];
            let blocks = encode(img, w, h);
            assert_eq!(
                blocks.len(),
                w.div_ceil(4) * h.div_ceil(4) * BLOCK_BYTES,
                "{w}×{h}"
            );
            assert_eq!(decode(&blocks, w, h).len(), w * h * 4);
        }
        assert!(encode(&[], 0, 0).is_empty());
        // A 1×1 flat image is a flat block, exact.
        assert_eq!(round_trip(&[19, 36, 53, 255], 1, 1), [19, 36, 53, 255]);
    }

    #[test]
    fn encoding_is_deterministic() {
        let img = noisy(64, 64);
        assert_eq!(encode(&img, 64, 64), encode(&img, 64, 64));
    }

    #[test]
    fn a_megatexel_encodes_at_the_recorded_quality() {
        // The time it takes is gm-ingest's record (tests/ingest.rs, the official avatar):
        // the browser build's gate greps this crate for std's clock.
        let (w, h) = (1024, 1024);
        let img = noisy(w, h);
        let blocks = encode(&img, w, h);
        let db = psnr(&img, &decode(&blocks, w, h));
        println!(
            "etc2 {w}×{h}: {db:.2} dB, {} bytes for {} raw ({:.1}:1)",
            blocks.len(),
            img.len(),
            img.len() as f64 / blocks.len() as f64
        );
        assert!(db > 30.0, "{db} dB");
    }
}
