//! Added and deleted line counts read from blob content, matching libgit2's
//! `Diff::stats` without materializing the patch that command counts.
//!
//! Parity rests on xdiff's details: records carry their trailing newline,
//! unmatchable records are pre-marked changed, and Myers follows
//! `xdl_split`'s heuristic even when that yields a non-minimal edit script.

use std::collections::HashMap;

use git2::{DiffDelta, DiffFile, Repository};

/// libgit2 `git_str_is_binary`: any NUL, a UTF-16 or UTF-32 BOM, or fewer
/// than one printable character per 128 non-printable ones.
pub(crate) fn is_binary(content: &[u8]) -> bool {
    let mut scan = 0usize;
    if content.len() >= 3 && content[0..3] == [0xEF, 0xBB, 0xBF] {
        scan = 3;
    }
    let wide_bom = (content.len() >= 4
        && (content[0..4] == [0x00, 0x00, 0xFE, 0xFF]
            || content[0..4] == [0xFF, 0xFE, 0x00, 0x00]))
        || (content.len() >= 2 && (content[0..2] == [0xFF, 0xFE] || content[0..2] == [0xFE, 0xFF]));
    if wide_bom {
        return true;
    }

    let (mut printable, mut nonprintable) = (0u32, 0u32);
    for &byte in &content[scan..] {
        if (byte > 0x1F && byte != 127) || byte == 0x08 || byte == 0x1B || byte == 0x0C {
            printable += 1;
        } else if byte == 0 {
            return true;
        } else if !matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | b'\r') {
            nonprintable += 1;
        }
    }
    (printable >> 7) < nonprintable
}

const XDL_MAX_COST_MIN: i64 = 256;
const XDL_HEUR_MIN_COST: i64 = 256;
const XDL_SNAKE_CNT: i64 = 20;
const XDL_K_HEUR: i64 = 4;
const XDL_LINE_MAX: i64 = i64::MAX;
const XDL_MAX_EQLIMIT: i64 = 1024;
const XDL_SIMSCAN_WINDOW: i64 = 100;
const XDL_KPDIS_RUN: i64 = 4;

/// xdiff's line split: each record runs to and includes its `\n`, and a file
/// that ends without a newline still yields a final record. A trailing newline
/// therefore makes the last record differ from the same line without one.
fn split_records(content: &[u8]) -> Vec<&[u8]> {
    let mut records = Vec::new();
    let mut start = 0usize;
    for (idx, byte) in content.iter().enumerate() {
        if *byte == b'\n' {
            records.push(&content[start..=idx]);
            start = idx + 1;
        }
    }
    if start < content.len() {
        records.push(&content[start..]);
    }
    records
}

/// xdiff's classical integer square root approximation (`xdl_bogosqrt`).
fn bogosqrt(mut n: i64) -> i64 {
    let mut value = 1i64;
    while n > 0 {
        value <<= 1;
        n >>= 2;
    }
    value
}

fn diagonal_index(d: i64, offset: i64) -> usize {
    (d + offset) as usize
}

struct Split {
    i1: i64,
    i2: i64,
    min_lo: bool,
    min_hi: bool,
}

/// Direct port of xdiff's `xdl_split`: forward and backward Myers frontiers
/// that may stop early once the edit cost passes the heuristic trigger.
#[allow(clippy::too_many_arguments)]
fn xdl_split(
    ha1: &[usize],
    ha2: &[usize],
    off1: i64,
    lim1: i64,
    off2: i64,
    lim2: i64,
    kvdf: &mut [i64],
    kvdb: &mut [i64],
    offset: i64,
    need_min: bool,
    mxcost: i64,
) -> Split {
    let dmin = off1 - lim2;
    let dmax = lim1 - off2;
    let fmid = off1 - off2;
    let bmid = lim1 - lim2;
    let odd = (fmid - bmid) & 1 != 0;
    let mut fmin = fmid;
    let mut fmax = fmid;
    let mut bmin = bmid;
    let mut bmax = bmid;

    kvdf[diagonal_index(fmid, offset)] = off1;
    kvdb[diagonal_index(bmid, offset)] = lim1;

    let mut ec = 1i64;
    loop {
        let mut got_snake = false;

        if fmin > dmin {
            fmin -= 1;
            kvdf[diagonal_index(fmin - 1, offset)] = -1;
        } else {
            fmin += 1;
        }
        if fmax < dmax {
            fmax += 1;
            kvdf[diagonal_index(fmax + 1, offset)] = -1;
        } else {
            fmax -= 1;
        }

        let mut d = fmax;
        while d >= fmin {
            let mut i1 =
                if kvdf[diagonal_index(d - 1, offset)] >= kvdf[diagonal_index(d + 1, offset)] {
                    kvdf[diagonal_index(d - 1, offset)] + 1
                } else {
                    kvdf[diagonal_index(d + 1, offset)]
                };
            let prev1 = i1;
            let mut i2 = i1 - d;
            while i1 < lim1 && i2 < lim2 && ha1[i1 as usize] == ha2[i2 as usize] {
                i1 += 1;
                i2 += 1;
            }
            if i1 - prev1 > XDL_SNAKE_CNT {
                got_snake = true;
            }
            kvdf[diagonal_index(d, offset)] = i1;
            if odd && bmin <= d && d <= bmax && kvdb[diagonal_index(d, offset)] <= i1 {
                return Split {
                    i1,
                    i2,
                    min_lo: true,
                    min_hi: true,
                };
            }
            d -= 2;
        }

        if bmin > dmin {
            bmin -= 1;
            kvdb[diagonal_index(bmin - 1, offset)] = XDL_LINE_MAX;
        } else {
            bmin += 1;
        }
        if bmax < dmax {
            bmax += 1;
            kvdb[diagonal_index(bmax + 1, offset)] = XDL_LINE_MAX;
        } else {
            bmax -= 1;
        }

        d = bmax;
        while d >= bmin {
            let mut i1 =
                if kvdb[diagonal_index(d - 1, offset)] < kvdb[diagonal_index(d + 1, offset)] {
                    kvdb[diagonal_index(d - 1, offset)]
                } else {
                    kvdb[diagonal_index(d + 1, offset)] - 1
                };
            let prev1 = i1;
            let mut i2 = i1 - d;
            while i1 > off1 && i2 > off2 && ha1[(i1 - 1) as usize] == ha2[(i2 - 1) as usize] {
                i1 -= 1;
                i2 -= 1;
            }
            if prev1 - i1 > XDL_SNAKE_CNT {
                got_snake = true;
            }
            kvdb[diagonal_index(d, offset)] = i1;
            if !odd && fmin <= d && d <= fmax && i1 <= kvdf[diagonal_index(d, offset)] {
                return Split {
                    i1,
                    i2,
                    min_lo: true,
                    min_hi: true,
                };
            }
            d -= 2;
        }

        if need_min {
            ec += 1;
            continue;
        }

        if got_snake && ec > XDL_HEUR_MIN_COST {
            let (mut best, mut pick) = (0i64, None);
            d = fmax;
            while d >= fmin {
                let dd = if d > fmid { d - fmid } else { fmid - d };
                let i1 = kvdf[diagonal_index(d, offset)];
                let i2 = i1 - d;
                let v = (i1 - off1) + (i2 - off2) - dd;
                if v > XDL_K_HEUR * ec
                    && v > best
                    && off1 + XDL_SNAKE_CNT <= i1
                    && i1 < lim1
                    && off2 + XDL_SNAKE_CNT <= i2
                    && i2 < lim2
                {
                    let mut k = 1i64;
                    while ha1[(i1 - k) as usize] == ha2[(i2 - k) as usize] {
                        if k == XDL_SNAKE_CNT {
                            best = v;
                            pick = Some((i1, i2));
                            break;
                        }
                        k += 1;
                    }
                }
                d -= 2;
            }
            if let Some((i1, i2)) = pick {
                return Split {
                    i1,
                    i2,
                    min_lo: true,
                    min_hi: false,
                };
            }

            let (mut best, mut pick) = (0i64, None);
            d = bmax;
            while d >= bmin {
                let dd = if d > bmid { d - bmid } else { bmid - d };
                let i1 = kvdb[diagonal_index(d, offset)];
                let i2 = i1 - d;
                let v = (lim1 - i1) + (lim2 - i2) - dd;
                if v > XDL_K_HEUR * ec
                    && v > best
                    && off1 < i1
                    && i1 <= lim1 - XDL_SNAKE_CNT
                    && off2 < i2
                    && i2 <= lim2 - XDL_SNAKE_CNT
                {
                    let mut k = 0i64;
                    while ha1[(i1 + k) as usize] == ha2[(i2 + k) as usize] {
                        if k == XDL_SNAKE_CNT - 1 {
                            best = v;
                            pick = Some((i1, i2));
                            break;
                        }
                        k += 1;
                    }
                }
                d -= 2;
            }
            if let Some((i1, i2)) = pick {
                return Split {
                    i1,
                    i2,
                    min_lo: false,
                    min_hi: true,
                };
            }
        }

        if ec >= mxcost {
            let (mut fbest, mut fbest1) = (-1i64, -1i64);
            d = fmax;
            while d >= fmin {
                let mut i1 = kvdf[diagonal_index(d, offset)].min(lim1);
                let mut i2 = i1 - d;
                if lim2 < i2 {
                    i1 = lim2 + d;
                    i2 = lim2;
                }
                if fbest < i1 + i2 {
                    fbest = i1 + i2;
                    fbest1 = i1;
                }
                d -= 2;
            }

            let (mut bbest, mut bbest1) = (XDL_LINE_MAX, XDL_LINE_MAX);
            d = bmax;
            while d >= bmin {
                let mut i1 = kvdb[diagonal_index(d, offset)].max(off1);
                let mut i2 = i1 - d;
                if i2 < off2 {
                    i1 = off2 + d;
                    i2 = off2;
                }
                if i1 + i2 < bbest {
                    bbest = i1 + i2;
                    bbest1 = i1;
                }
                d -= 2;
            }

            return if (lim1 + lim2) - bbest < fbest - (off1 + off2) {
                Split {
                    i1: fbest1,
                    i2: fbest - fbest1,
                    min_lo: true,
                    min_hi: false,
                }
            } else {
                Split {
                    i1: bbest1,
                    i2: bbest - bbest1,
                    min_lo: false,
                    min_hi: true,
                }
            };
        }

        ec += 1;
    }
}

/// Direct port of xdiff's `xdl_recs_cmp`. Records are marked changed through
/// `rindex`, which maps a kept index back to its original line.
#[allow(clippy::too_many_arguments)]
fn recs_cmp(
    ha1: &[usize],
    ha2: &[usize],
    rindex1: &[usize],
    rindex2: &[usize],
    rchg1: &mut [bool],
    rchg2: &mut [bool],
    kvdf: &mut [i64],
    kvdb: &mut [i64],
    offset: i64,
    mxcost: i64,
) {
    let mut stack = vec![(0i64, ha1.len() as i64, 0i64, ha2.len() as i64, false)];
    while let Some((mut off1, mut lim1, mut off2, mut lim2, need_min)) = stack.pop() {
        while off1 < lim1 && off2 < lim2 && ha1[off1 as usize] == ha2[off2 as usize] {
            off1 += 1;
            off2 += 1;
        }
        while off1 < lim1 && off2 < lim2 && ha1[(lim1 - 1) as usize] == ha2[(lim2 - 1) as usize] {
            lim1 -= 1;
            lim2 -= 1;
        }

        if off1 == lim1 {
            for i in off2..lim2 {
                rchg2[rindex2[i as usize]] = true;
            }
        } else if off2 == lim2 {
            for i in off1..lim1 {
                rchg1[rindex1[i as usize]] = true;
            }
        } else {
            let spl = xdl_split(
                ha1, ha2, off1, lim1, off2, lim2, kvdf, kvdb, offset, need_min, mxcost,
            );
            stack.push((off1, spl.i1, off2, spl.i2, spl.min_lo));
            stack.push((spl.i1, lim1, spl.i2, lim2, spl.min_hi));
        }
    }
}

/// xdiff's `xdl_clean_mmatch`: a multi-match line is discarded as changed only
/// when it sits between runs of unmatchable lines.
fn clean_mmatch(dis: &[i64], i: i64, mut s: i64, mut e: i64) -> bool {
    if i - s > XDL_SIMSCAN_WINDOW {
        s = i - XDL_SIMSCAN_WINDOW;
    }
    if e - i > XDL_SIMSCAN_WINDOW {
        e = i + XDL_SIMSCAN_WINDOW;
    }

    let (mut rdis0, mut rpdis0) = (0i64, 1i64);
    let mut r = 1i64;
    while i - r >= s {
        if dis[(i - r) as usize] == 0 {
            rdis0 += 1;
        } else if dis[(i - r) as usize] == 2 {
            rpdis0 += 1;
        } else {
            break;
        }
        r += 1;
    }
    if rdis0 == 0 {
        return false;
    }

    let (mut rdis1, mut rpdis1) = (0i64, 1i64);
    r = 1;
    while i + r <= e {
        if dis[(i + r) as usize] == 0 {
            rdis1 += 1;
        } else if dis[(i + r) as usize] == 2 {
            rpdis1 += 1;
        } else {
            break;
        }
        r += 1;
    }
    if rdis1 == 0 {
        return false;
    }

    rdis1 += rdis0;
    rpdis1 += rpdis0;
    rpdis1 * XDL_KPDIS_RUN < rpdis1 + rdis1
}

/// One side of a delta, reduced the way `xdl_prepare_env` reduces a file:
/// aligned ends trimmed, then unmatchable records pre-marked changed and
/// dropped from the sequence Myers runs over.
struct SideData {
    changed: Vec<bool>,
    kept: Vec<usize>,
}

fn classify<'a>(
    records: &[&'a [u8]],
    classes: &mut HashMap<&'a [u8], usize>,
    class: &mut Vec<usize>,
    counts: &mut Vec<[u64; 2]>,
    which: usize,
) {
    for record in records {
        let id = match classes.get(*record) {
            Some(id) => *id,
            None => {
                let id = counts.len();
                classes.insert(*record, id);
                counts.push([0, 0]);
                id
            }
        };
        counts[id][which] += 1;
        class.push(id);
    }
}

/// xdiff's `xdl_cleanup_records` for one side.
fn reduce(
    class: &[usize],
    counts: &[[u64; 2]],
    dstart: usize,
    dend: i64,
    other_index: usize,
) -> SideData {
    let nrec = class.len();
    let mut changed = vec![false; nrec];
    let mut mlim = bogosqrt(nrec as i64);
    if mlim > XDL_MAX_EQLIMIT {
        mlim = XDL_MAX_EQLIMIT;
    }

    let mut dis = vec![0i64; nrec];
    for (i, &id) in class.iter().enumerate() {
        if (i as i64) < dstart as i64 || (i as i64) > dend {
            continue;
        }
        let nm = counts[id][other_index] as i64;
        dis[i] = if nm == 0 {
            0
        } else if nm >= mlim {
            2
        } else {
            1
        };
    }

    let mut kept = Vec::new();
    for i in dstart as i64..=dend {
        let i = i as usize;
        let keep =
            dis[i] == 1 || (dis[i] == 2 && !clean_mmatch(&dis, i as i64, dstart as i64, dend));
        if keep {
            kept.push(i);
        } else {
            changed[i] = true;
        }
    }

    SideData { changed, kept }
}

/// `(insertions, deletions)` for one file delta, matching libgit2's
/// `Diff::stats` line counts without building a patch.
pub fn delta_numstat(repo: &Repository, delta: &DiffDelta<'_>) -> (u64, u64) {
    let old_blob = side(repo, delta.old_file());
    let new_blob = side(repo, delta.new_file());
    let old = old_blob.as_ref().map_or(&[][..], |blob| blob.content());
    let new = new_blob.as_ref().map_or(&[][..], |blob| blob.content());

    // A mode-only change keeps both blobs identical.
    if old == new {
        return (0, 0);
    }
    if is_binary(old) || is_binary(new) {
        return (0, 0);
    }

    let old_records = split_records(old);
    let new_records = split_records(new);

    let mut classes: HashMap<&[u8], usize> = HashMap::new();
    let mut counts: Vec<[u64; 2]> = Vec::new();
    let mut old_class = Vec::with_capacity(old_records.len());
    let mut new_class = Vec::with_capacity(new_records.len());
    classify(&old_records, &mut classes, &mut old_class, &mut counts, 0);
    classify(&new_records, &mut classes, &mut new_class, &mut counts, 1);

    let (start, suffix) = trim_ends(&old_class, &new_class);
    let old_dend = (old_records.len() as i64) - suffix as i64 - 1;
    let new_dend = (new_records.len() as i64) - suffix as i64 - 1;
    let old_side = reduce(&old_class, &counts, start, old_dend, 1);
    let new_side = reduce(&new_class, &counts, start, new_dend, 0);

    let ha1: Vec<usize> = old_side.kept.iter().map(|&i| old_class[i]).collect();
    let ha2: Vec<usize> = new_side.kept.iter().map(|&i| new_class[i]).collect();
    let nreff1 = ha1.len() as i64;
    let nreff2 = ha2.len() as i64;
    let mut kvdf = vec![0i64; (nreff1 + nreff2 + 3) as usize];
    let mut kvdb = vec![0i64; (nreff1 + nreff2 + 3) as usize];
    let offset = nreff2 + 1;
    let mxcost = bogosqrt(nreff1 + nreff2 + 3).max(XDL_MAX_COST_MIN);

    let mut old_changed = old_side.changed;
    let mut new_changed = new_side.changed;
    recs_cmp(
        &ha1,
        &ha2,
        &old_side.kept,
        &new_side.kept,
        &mut old_changed,
        &mut new_changed,
        &mut kvdf,
        &mut kvdb,
        offset,
        mxcost,
    );

    let insertions = new_changed.iter().filter(|c| **c).count() as u64;
    let deletions = old_changed.iter().filter(|c| **c).count() as u64;
    (insertions, deletions)
}

/// xdiff's `xdl_trim_ends`: matching records at both ends are never changed,
/// so Myers only has to run over the middle. Returns the shared start and the
/// suffix length, which yield each side's end independently.
fn trim_ends(old_class: &[usize], new_class: &[usize]) -> (usize, usize) {
    let limit = old_class.len().min(new_class.len());
    let mut prefix = 0usize;
    while prefix < limit && old_class[prefix] == new_class[prefix] {
        prefix += 1;
    }
    let remainder = limit - prefix;
    let mut suffix = 0usize;
    while suffix < remainder
        && old_class[old_class.len() - 1 - suffix] == new_class[new_class.len() - 1 - suffix]
    {
        suffix += 1;
    }
    (prefix, suffix)
}

/// Blob for one side of a delta. An absent side has no blob, which is what
/// makes an added file all insertions and a deleted one all deletions.
fn side<'repo>(repo: &'repo Repository, file: DiffFile<'_>) -> Option<git2::Blob<'repo>> {
    if file.id() == git2::Oid::ZERO_SHA1 {
        return None;
    }
    repo.find_blob(file.id()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting_follows_xdiffs_record_rule() {
        assert!(split_records(b"").is_empty());
        assert_eq!(split_records(b"\n"), vec![&b"\n"[..]]);
        assert_eq!(split_records(b"a\nb"), vec![&b"a\n"[..], &b"b"[..]]);
        assert_eq!(split_records(b"a\nb\n"), vec![&b"a\n"[..], &b"b\n"[..]]);
        assert_eq!(split_records(b"a"), vec![&b"a"[..]]);
        assert_eq!(
            split_records(b"a\r\nb\r\n"),
            vec![&b"a\r\n"[..], &b"b\r\n"[..]]
        );
    }

    #[test]
    fn binary_detection_follows_libgit2() {
        assert!(!is_binary(b"plain ascii text\n"));
        assert!(!is_binary(b"tabs\tand\ttabs\n"));
        // UTF-8 BOM is skipped rather than treated as binary.
        assert!(!is_binary(b"\xEF\xBB\xBFwith a bom"));
        assert!(is_binary(b"\xFF\xFE\x00\x00utf-32le"));
        assert!(is_binary(b"\xFE\xFFutf-16be"));
        assert!(is_binary(b"embedded\0nul"));
        // 128 printable characters are needed to outweigh one non-printable.
        let lopsided = vec![b'a'; 100]
            .into_iter()
            .chain([0x01])
            .collect::<Vec<u8>>();
        assert!(is_binary(&lopsided));
        let balanced = vec![b'a'; 200]
            .into_iter()
            .chain([0x01])
            .collect::<Vec<u8>>();
        assert!(!is_binary(&balanced));
    }

    #[test]
    fn bogosqrt_matches_reference_ladder() {
        assert_eq!(bogosqrt(0), 1);
        assert_eq!(bogosqrt(1), 2);
        assert_eq!(bogosqrt(4), 4);
        assert_eq!(bogosqrt(5), 4);
        assert_eq!(bogosqrt(16), 8);
        assert_eq!(bogosqrt(17), 8);
    }

    struct Snapshot {
        _dir: tempfile::TempDir,
        repo: git2::Repository,
    }

    impl Snapshot {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let repo = git2::Repository::init(dir.path()).unwrap();
            Self { _dir: dir, repo }
        }

        fn diff(&self, old: &[u8], new: &[u8]) -> git2::Diff<'_> {
            let old_oid = self.repo.blob(old).unwrap();
            let new_oid = self.repo.blob(new).unwrap();
            let mut old_tb = self.repo.treebuilder(None).unwrap();
            old_tb.insert("f", old_oid, 0o100644).unwrap();
            let old_tree = self.repo.find_tree(old_tb.write().unwrap()).unwrap();
            let mut new_tb = self.repo.treebuilder(None).unwrap();
            new_tb.insert("f", new_oid, 0o100644).unwrap();
            let new_tree = self.repo.find_tree(new_tb.write().unwrap()).unwrap();
            self.repo
                .diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
                .unwrap()
        }

        fn assert_matches_libgit2(&self, old: &[u8], new: &[u8]) {
            let diff = self.diff(old, new);
            let mut deltas = diff.deltas();
            let Some(delta) = deltas.next() else {
                return;
            };
            let expected = {
                let stats = diff.stats().unwrap();
                (stats.insertions() as u64, stats.deletions() as u64)
            };
            assert_eq!(
                delta_numstat(&self.repo, &delta),
                expected,
                "old={old:?} new={new:?}"
            );
        }
    }

    #[test]
    fn final_newline_change_is_a_change() {
        let snap = Snapshot::new();
        snap.assert_matches_libgit2(b"a\nb", b"a\nb\n");
        snap.assert_matches_libgit2(b"a\nb\n", b"a\nb");
        snap.assert_matches_libgit2(b"a", b"a\n");
    }

    #[test]
    fn empty_and_binary_pairs_match_libgit2() {
        let snap = Snapshot::new();
        snap.assert_matches_libgit2(b"", b"");
        snap.assert_matches_libgit2(b"", b"a\n");
        snap.assert_matches_libgit2(b"a\n", b"");
        snap.assert_matches_libgit2(b"\n", b"");

        let binary: Vec<u8> = std::iter::once(0u8)
            .chain(1u8..=64)
            .chain(std::iter::repeat_n(b'A', 100))
            .collect();
        snap.assert_matches_libgit2(b"text\n", &binary);
        snap.assert_matches_libgit2(&binary, b"text\n");
    }

    #[test]
    fn random_line_edits_match_libgit2() {
        let snap = Snapshot::new();
        let mut state = 0x2545F4914F6CDD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let words = ["alpha", "beta", "gamma", "delta", "x"];
        for _ in 0..2000 {
            let base: Vec<String> = (0..next() % 40)
                .map(|_| words[(next() % words.len() as u64) as usize].to_owned())
                .collect();
            let mut edited = base.clone();
            let mutations = next() % 6;
            for _ in 0..mutations {
                if edited.is_empty() {
                    edited.push(words[(next() % words.len() as u64) as usize].to_owned());
                    continue;
                }
                let at = (next() % edited.len() as u64) as usize;
                match next() % 3 {
                    0 => {
                        edited.remove(at);
                    }
                    1 => {
                        edited.insert(at, words[(next() % words.len() as u64) as usize].to_owned())
                    }
                    _ => edited[at] = words[(next() % words.len() as u64) as usize].to_owned(),
                }
            }
            let join = |lines: &[String]| lines.join("\n") + "\n";
            let old = join(&base);
            let new = join(&edited);
            // Toggle the trailing newline on some iterations.
            let old = if next() % 2 == 0 {
                old
            } else {
                old.trim_end_matches('\n').to_owned()
            };
            let new = if next() % 2 == 0 {
                new
            } else {
                new.trim_end_matches('\n').to_owned()
            };
            snap.assert_matches_libgit2(old.as_bytes(), new.as_bytes());
        }
    }

    /// Larger, more repetitive inputs exercise the `xdl_cleanup_records`
    /// multi-match discard and the `xdl_split` heuristic.
    #[test]
    fn repetitive_large_inputs_match_libgit2() {
        let snap = Snapshot::new();
        let mut state = 0x9E3779B97F4A7C15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..60 {
            let size = 800 + (next() % 1800) as usize;
            let base: Vec<String> = (0..size).map(|_| format!("l{}", next() % 8)).collect();
            let mut edited = base.clone();
            for _ in 0..(next() % 400) {
                if edited.is_empty() {
                    break;
                }
                let at = (next() % edited.len() as u64) as usize;
                match next() % 3 {
                    0 => {
                        edited.remove(at);
                    }
                    1 => edited.insert(at, format!("l{}", next() % 8)),
                    _ => edited[at] = format!("l{}", next() % 8),
                }
            }
            let old = base.join("\n") + "\n";
            let new = edited.join("\n") + "\n";
            snap.assert_matches_libgit2(old.as_bytes(), new.as_bytes());
        }
    }
}
