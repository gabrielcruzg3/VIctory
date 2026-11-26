//! Title bytes scattered over a page of a copy in the card's memory go up
//! as the page's bytes and a mask, applied by one compute pass, instead of
//! one copy a run (`VICTORY_MERGE_MASK`; off unless set).
//!
//! Where it matters: under flights' shader writes the title's bytes are put
//! into a copy run by run ([`crate::shadow::Shadow::merge_under_writes`]),
//! since a flight may have written the bytes in between on the card. Each
//! run became a `CopyBufferRegion` of its own in the list that brings the
//! card side up to date (`Host::card_before`), and a hash pass of its own
//! over its 64 KiB piece. A page the title rewrote with a few bytes changed
//! here and there holds thousands of runs (up to about 16,000 in fork issue
//! #1083), so the merge stops at `VICTORY_MERGE_RUNS` runs a submission and
//! settles the flights instead: 27-35 ms a frame of the submitting thread
//! there.
//!
//! With the switch, a page (a 64 KiB block of title memory, the unit the
//! write tracking and the merge use) with more changed ranges than the
//! threshold costs the merge's budget one run, and on the card it is one
//! group of threads of [`CARD_MASK`]: a thread a dword reads the copy's
//! CPU side (what a copy would have copied from) and writes the bytes the
//! mask names, keeping the others, which may be a flight's. The bytes the
//! card side ends with are the ones the copies would have left: every
//! masked byte from the CPU side, every other byte as it was. The ranges
//! themselves are kept as before (the relay's merge skips them), and the
//! page's piece is hashed again once.
//!
//! Not with the changed bytes sent through the copy queue
//! (`VICTORY_QUEUE2` bit 2): those land packed, away from their offsets.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// A block: the 64 KiB of title memory the write tracking and the merge
/// work in.
pub(crate) const BLOCK: usize = crate::watch::PAGE;

/// The bytes of one block's mask: a bit a byte.
pub(crate) const MASK_BYTES: usize = BLOCK / 8;

/// The bytes of a block's entry in the table ([`CARD_MASK`]).
pub(crate) const ENTRY: usize = 16;

/// Threads in a group of [`CARD_MASK`].
pub(crate) const THREADS: usize = 256;

/// The threshold `VICTORY_MERGE_MASK=1` stands for: blocks with more
/// changed ranges than this go up with a mask. From `mask_bench` (RTX 4060
/// Laptop, Intel UHD, WARP): from 16 ranges a block up the pass took no
/// more of the submitting thread than the copies on any of them, and at 64
/// or more less on all; below 16 the two were within noise. On the card the
/// pass costs about one hash pass of the block; with the hash passes joined
/// (`VICTORY_CARD_REHASH_RUNS=1`) WARP's copies stay cheaper up to about 64.
pub(crate) const DEFAULT_THRESHOLD: usize = 16;

/// Whether the host made [`CARD_MASK`]'s pipeline.
static READY: AtomicBool = AtomicBool::new(false);

/// Since the last frame line: blocks that went up with a mask, and the
/// changed ranges they stood for.
pub(crate) static STATS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

pub(crate) fn set_ready() {
    READY.store(true, Ordering::Relaxed);
}

/// The switch: `VICTORY_MERGE_MASK` unset or `0` off; `1` on with
/// [`DEFAULT_THRESHOLD`]; a number from 2 up on with that threshold;
/// `alt`: twenty seconds on, twenty off, in turn, each change said in the
/// log, so that one run measures both in the same scene.
fn switch() -> Option<usize> {
    /// 0 off, `usize::MAX` in turn, else the threshold.
    static MODE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let mode = *MODE.get_or_init(|| {
        match std::env::var("VICTORY_MERGE_MASK")
            .as_deref()
            .map(str::trim)
        {
            Ok("alt") => usize::MAX,
            Ok(v) => match v.parse::<usize>() {
                Ok(0) | Err(_) => 0,
                Ok(1) => DEFAULT_THRESHOLD,
                Ok(n) => n,
            },
            Err(_) => 0,
        }
    });
    match mode {
        0 => None,
        usize::MAX => {
            static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let on = (EPOCH
                .get_or_init(std::time::Instant::now)
                .elapsed()
                .as_secs()
                / 20)
                % 2
                == 1;
            static LAST: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(2);
            if LAST.swap(u8::from(on), Ordering::Relaxed) != u8::from(on) {
                tracing::info!(
                    "pages with many changed runs go up to the card with a mask from now (VICTORY_MERGE_MASK=alt): {}",
                    if on {
                        "ON"
                    } else {
                        "off, a copy a run as before"
                    }
                );
            }
            on.then_some(DEFAULT_THRESHOLD)
        }
        n => Some(n),
    }
}

/// The threshold in force, if the masked upload can be used at all: the
/// switch is on, the pipeline was made, and the changed bytes are not sent
/// through the copy queue.
pub(crate) fn threshold() -> Option<usize> {
    let n = switch()?;
    if !READY.load(Ordering::Relaxed) || crate::queue2::second().is_some_and(|s| s.uploads()) {
        return None;
    }
    Some(n)
}

/// Whether the blocks of a copy whose title buffer starts at `start` and
/// whose buffers hold `size` bytes can be masked: dword-aligned offsets,
/// and the last dword a block may touch lies inside the buffers.
pub(crate) fn eligible(start: usize, size: u64) -> bool {
    start.is_multiple_of(4) && size.is_multiple_of(4)
}

/// What one page's changed runs cost the merge's budget
/// ([`crate::shadow::Shadow::merge_under_writes`]): one if they go up with
/// a mask, else one a run.
pub(crate) fn cost(runs: usize, start: usize, size: u64) -> usize {
    match threshold() {
        Some(n) if runs > n && eligible(start, size) => 1,
        _ => runs,
    }
}

/// One block that goes up with a mask: its first offset in the buffer
/// (4-aligned), the dwords from there the pass looks at, and its pieces of
/// the changed ranges ([`Split::pieces`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Masked {
    pub(crate) lo: usize,
    pub(crate) dwords: usize,
    pub(crate) pieces: std::ops::Range<usize>,
}

/// A copy's changed ranges, split: those still copied, and the blocks that
/// go up with a mask.
#[derive(Debug, Default)]
pub(crate) struct Split {
    /// Copied as before, in order, joined where they touch.
    pub(crate) copies: Vec<(usize, usize)>,
    pub(crate) blocks: Vec<Masked>,
    /// The masked blocks' pieces of the ranges, cut at block edges.
    pub(crate) pieces: Vec<(usize, usize)>,
}

impl Split {
    /// What the hash passes go over afterwards: the copies and one range
    /// a masked block (the same 64 KiB pieces as all of its ranges).
    pub(crate) fn hashed(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = self.copies.clone();
        out.extend(self.blocks.iter().map(|b| {
            (
                self.pieces[b.pieces.start].0,
                self.pieces[b.pieces.end - 1].1,
            )
        }));
        out.sort_unstable();
        out
    }

    /// The changed ranges the masked blocks stand for.
    pub(crate) fn masked_ranges(&self) -> usize {
        self.pieces.len()
    }
}

/// Splits a copy's changed ranges (offsets in its buffer, in order, not
/// overlapping) by block: a block (`start` is the title buffer's address)
/// holding more than `threshold` of them goes up with a mask, the others'
/// ranges are copied.
pub(crate) fn split(ranges: &[(usize, usize)], start: usize, threshold: usize) -> Split {
    let mut out = Split::default();
    let block_of = |offset: usize| (start + offset) / BLOCK;
    // The pieces of the block being gathered.
    let mut group: Vec<(usize, usize)> = Vec::new();
    let mut group_block = usize::MAX;
    let flush = |out: &mut Split, group: &mut Vec<(usize, usize)>, block: usize| {
        if group.is_empty() {
            return;
        }
        if group.len() > threshold {
            let lo = (block * BLOCK).max(start) - start;
            let end = group.last().map_or(lo, |r| r.1);
            let first = out.pieces.len();
            out.pieces.append(group);
            out.blocks.push(Masked {
                lo,
                dwords: (end - lo).div_ceil(4),
                pieces: first..out.pieces.len(),
            });
        } else {
            for (a, b) in group.drain(..) {
                match out.copies.last_mut() {
                    Some(last) if last.1 == a => last.1 = b,
                    _ => out.copies.push((a, b)),
                }
            }
        }
    };
    for &(a, b) in ranges {
        let mut at = a;
        while at < b {
            let block = block_of(at);
            let edge = ((block + 1) * BLOCK - start).min(b);
            if block != group_block {
                flush(&mut out, &mut group, group_block);
                group_block = block;
            }
            group.push((at, edge));
            at = edge;
        }
    }
    flush(&mut out, &mut group, group_block);
    out
}

/// Writes a masked block's mask into `out` (`dwords.div_ceil(8)` dwords):
/// bit `k % 32` of dword `k / 32` is set where byte `lo + k` is in one of
/// `pieces`.
pub(crate) fn write_mask(lo: usize, dwords: usize, pieces: &[(usize, usize)], out: &mut [u32]) {
    let out = &mut out[..dwords.div_ceil(8)];
    out.fill(0);
    for &(a, b) in pieces {
        let (mut k, end) = (a - lo, b - lo);
        while k < end {
            let bit = k % 32;
            let n = (32 - bit).min(end - k);
            let bits = if n == 32 {
                u32::MAX
            } else {
                ((1u32 << n) - 1) << bit
            };
            out[k / 32] |= bits;
            k += n;
        }
    }
}

/// What [`CARD_MASK`] does, on the CPU, for tests: dword by dword, a dword
/// with mask bits written whole from `src` where they are set, the rest of
/// it kept.
#[cfg(test)]
pub(crate) fn apply(dst: &mut [u8], src: &[u8], block: &Masked, mask: &[u32]) {
    for i in 0..block.dwords {
        let m = (mask[i / 8] >> ((i % 8) * 4)) & 15;
        if m == 0 {
            continue;
        }
        let at = block.lo + i * 4;
        let d = u32::from_le_bytes(dst[at..at + 4].try_into().unwrap());
        let s = u32::from_le_bytes(src[at..at + 4].try_into().unwrap());
        let bm = (0..4)
            .filter(|k| m >> k & 1 != 0)
            .fold(0u32, |acc, k| acc | 0xff << (8 * k));
        dst[at..at + 4].copy_from_slice(&((d & !bm) | (s & bm)).to_le_bytes());
    }
}

/// The pass: a group of [`THREADS`] threads per 1,024 bytes along x, a
/// block per row along y. `table` holds per block (16 bytes): its first
/// byte in the copy, its dwords, and where its mask starts in `table`.
/// `src` is the copy's CPU side, `dst` its card side, at the same offsets.
/// A dword whose four bytes are all masked is written whole; one with some
/// masked is read and written back with only those changed. Nothing else
/// writes the card side while the pass runs (it is on the queue behind
/// every flight and ahead of the submission's lists).
pub(crate) const CARD_MASK: &str = r#"
#define RS "SRV(t0), SRV(t1), UAV(u0)"
ByteAddressBuffer src : register(t0);
ByteAddressBuffer table : register(t1);
RWByteAddressBuffer dst : register(u0);

[RootSignature(RS)]
[numthreads(256, 1, 1)]
void main(uint3 group : SV_GroupID, uint3 thread : SV_GroupThreadID) {
    uint3 entry = table.Load3(group.y * 16u);
    uint i = group.x * 256u + thread.x;
    if (i >= entry.y) return;
    uint m = (table.Load(entry.z + (i >> 3u) * 4u) >> ((i & 7u) * 4u)) & 15u;
    if (m == 0u) return;
    uint at = entry.x + i * 4u;
    uint s = src.Load(at);
    if (m == 15u) {
        dst.Store(at, s);
        return;
    }
    uint take = ((m & 1u) != 0u ? 0x000000FFu : 0u) | ((m & 2u) != 0u ? 0x0000FF00u : 0u)
              | ((m & 4u) != 0u ? 0x00FF0000u : 0u) | ((m & 8u) != 0u ? 0xFF000000u : 0u);
    dst.Store(at, (dst.Load(at) & ~take) | (s & take));
}
"#;

/// Bytes of upload memory one copy's masked blocks take
/// ([`write_section`]): the table, rounded up to 256, then a mask a block.
pub(crate) fn section_bytes(blocks: usize) -> usize {
    (blocks * ENTRY).next_multiple_of(256) + blocks * MASK_BYTES
}

/// Writes one copy's table and masks at `out` ([`section_bytes`] of them):
/// what [`CARD_MASK`] reads as `table`.
///
/// # Safety
/// `out` is writable for [`section_bytes`]`(split.blocks.len())` bytes and
/// 4-aligned.
pub(crate) unsafe fn write_section(split: &Split, out: *mut u8) {
    let masks_at = (split.blocks.len() * ENTRY).next_multiple_of(256);
    // Each mask is made in cached memory and then copied out whole: the
    // upload buffer is write-combined, and the bit-by-bit build read it
    // back (9 ms for four pages of 16,384 runs, against 0.1 ms).
    let mut mask = [0u32; MASK_BYTES / 4];
    for (k, block) in split.blocks.iter().enumerate() {
        let mask_at = masks_at + k * MASK_BYTES;
        let entry = [block.lo as u32, block.dwords as u32, mask_at as u32, 0];
        let words = block.dwords.div_ceil(8);
        write_mask(
            block.lo,
            block.dwords,
            &split.pieces[block.pieces.clone()],
            &mut mask,
        );
        // SAFETY: inside the section (the caller's promise); 4-aligned.
        unsafe {
            std::ptr::copy_nonoverlapping(entry.as_ptr().cast::<u8>(), out.add(k * ENTRY), ENTRY);
            std::ptr::copy_nonoverlapping(mask.as_ptr().cast::<u8>(), out.add(mask_at), words * 4);
        }
    }
}

/// The groups along x a pass over `split`'s blocks needs.
pub(crate) fn groups_x(split: &Split) -> u32 {
    split
        .blocks
        .iter()
        .map(|b| b.dwords.div_ceil(THREADS))
        .max()
        .unwrap_or(0) as u32
}

/// A xorshift generator for tests.
#[cfg(test)]
fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut s = seed | 1;
    move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    }
}

/// Random changed ranges over `len` bytes for tests: on the order of
/// `runs` of them, of 1 to `longest` bytes, apart by at least one byte.
#[cfg(test)]
fn random_ranges(
    next: &mut impl FnMut() -> u64,
    len: usize,
    runs: usize,
    longest: usize,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let stride = (len / runs.max(1)).max(2);
    let mut at = (next() as usize) % stride;
    while at < len {
        let n = 1 + (next() as usize) % longest.clamp(1, stride - 1);
        let end = (at + n).min(len);
        out.push((at, end));
        at = end + 1 + (next() as usize) % stride;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_leave_the_bytes_the_copies_would() {
        let mut next = rng(0x5eed);
        for case in 0..400 {
            let len = [BLOCK, 3 * BLOCK, 2 * BLOCK + 4096, 700][case % 4];
            // Title buffers starting at, across and after a block's edge.
            let start = [
                0x1000_0000usize,
                0x1000_0000 + BLOCK - 1024,
                0x1000_0000 + 40,
                0x2000_0000 + 2,
            ][(case / 4) % 4];
            let size = len.next_multiple_of(4) as u64;
            let runs = [1, 4, 16, 17, 256, 4000, 16000][case % 7];
            let longest = [1, 3, 9, 300][case % 4];
            let ranges = random_ranges(&mut next, len, runs, longest);
            let src: Vec<u8> = (0..size as usize).map(|_| next() as u8).collect();
            let card: Vec<u8> = (0..size as usize).map(|_| next() as u8).collect();
            let mut want = card.clone();
            for &(a, b) in &ranges {
                want[a..b].copy_from_slice(&src[a..b]);
            }
            for threshold in [0, 1, 16, 1 << 20] {
                let split = split(&ranges, start, threshold);
                let mut got = card.clone();
                for &(a, b) in &split.copies {
                    got[a..b].copy_from_slice(&src[a..b]);
                }
                for block in &split.blocks {
                    if eligible(start, size) {
                        assert!(block.lo.is_multiple_of(4));
                        assert!(
                            (block.lo + 4 * block.dwords) as u64 <= size,
                            "the pass stays in the buffer"
                        );
                    }
                    assert!(block.dwords <= BLOCK / 4);
                    let mut mask = vec![0u32; MASK_BYTES / 4];
                    write_mask(
                        block.lo,
                        block.dwords,
                        &split.pieces[block.pieces.clone()],
                        &mut mask,
                    );
                    if eligible(start, size) {
                        apply(&mut got, &src, block, &mask);
                    }
                }
                if eligible(start, size) {
                    assert_eq!(got, want, "case {case}, threshold {threshold}");
                }
                // Every changed byte is copied or masked, once.
                let mut covered: Vec<(usize, usize)> = split
                    .copies
                    .iter()
                    .chain(split.pieces.iter())
                    .copied()
                    .collect();
                covered.sort_unstable();
                let mut joined: Vec<(usize, usize)> = Vec::new();
                for (a, b) in covered {
                    match joined.last_mut() {
                        Some(last) if last.1 == a => last.1 = b,
                        _ => joined.push((a, b)),
                    }
                }
                assert_eq!(joined, ranges, "case {case}, threshold {threshold}");
                if threshold == 1 << 20 {
                    assert!(split.blocks.is_empty());
                }
            }
        }
    }

    #[test]
    fn a_block_with_few_ranges_is_copied() {
        let ranges = vec![(0, 2), (10, 12), (BLOCK + 8, BLOCK + 9)];
        let split = split(&ranges, 0x4000_0000, 1);
        assert_eq!(split.copies, vec![(BLOCK + 8, BLOCK + 9)]);
        assert_eq!(
            split.blocks,
            vec![Masked {
                lo: 0,
                dwords: 3,
                pieces: 0..2
            }]
        );
        assert_eq!(split.hashed(), vec![(0, 12), (BLOCK + 8, BLOCK + 9)]);
        // A range over a block's edge: its piece in a copied block joins
        // the one in the next copied block again.
        let split = super::split(&[(BLOCK - 4, BLOCK + 4)], 0x4000_0000, 1);
        assert_eq!(split.copies, vec![(BLOCK - 4, BLOCK + 4)]);
    }
}
