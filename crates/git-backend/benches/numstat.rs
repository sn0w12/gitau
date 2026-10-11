//! Line-count benchmarks for the git2 `numstat` path against libgit2's
//! `Diff::stats`, on the shapes the history chart sees: a one-line commit, a
//! small edit, and a large repetitive edit where Myers actually runs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::engines::git2::numstat::delta_numstat;
use git2::Repository;

struct Content {
    name: &'static str,
    old: Vec<u8>,
    new: Vec<u8>,
}

/// A small xorshift so the fixtures are deterministic across runs.
fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut state = seed;
    move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    }
}

fn edited_file(lines: usize, edits: usize, vocabulary: usize, seed: u64) -> (Vec<u8>, Vec<u8>) {
    let mut next = rng(seed);
    let words: Vec<String> = (0..vocabulary)
        .map(|i| format!("    let value_{} = compute({}, {});\n", i, i, i + 1))
        .collect();
    let base: Vec<String> = (0..lines)
        .map(|_| words[(next() % vocabulary as u64) as usize].clone())
        .collect();
    let mut edited = base.clone();
    for _ in 0..edits {
        let at = (next() % edited.len() as u64) as usize;
        edited[at] = words[(next() % vocabulary as u64) as usize].clone();
    }
    (base.concat().into_bytes(), edited.concat().into_bytes())
}

fn contents() -> Vec<Content> {
    let mut out = vec![Content {
        name: "tiny-1line",
        old: b"content for revision 0\n".to_vec(),
        new: b"content for revision 1\n".to_vec(),
    }];

    // Changed lines repeat elsewhere in the file, so cleanup cannot discard
    // them and Myers has to run on repeated content.
    let (small_old, small_new) = edited_file(2_000, 100, 400, 0xDEAD_BEEF_CAFE_BABE);
    out.push(Content {
        name: "small-2k",
        old: small_old,
        new: small_new,
    });

    let (large_old, large_new) = edited_file(20_000, 4_000, 400, 0x9E37_79B9_7F4A_7C15);
    out.push(Content {
        name: "large-20k",
        old: large_old,
        new: large_new,
    });

    out
}

fn build_diff<'repo>(repo: &'repo Repository, old: &[u8], new: &[u8]) -> git2::Diff<'repo> {
    let old_oid = repo.blob(old).unwrap();
    let new_oid = repo.blob(new).unwrap();
    let mut old_tb = repo.treebuilder(None).unwrap();
    old_tb.insert("file.txt", old_oid, 0o100644).unwrap();
    let old_tree = repo.find_tree(old_tb.write().unwrap()).unwrap();
    let mut new_tb = repo.treebuilder(None).unwrap();
    new_tb.insert("file.txt", new_oid, 0o100644).unwrap();
    let new_tree = repo.find_tree(new_tb.write().unwrap()).unwrap();
    repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None)
        .unwrap()
}

fn bench_numstat(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    for content in contents() {
        let diff = build_diff(&repo, &content.old, &content.new);
        let mut deltas = diff.deltas();
        let delta = deltas.next().unwrap();

        c.bench_function(&format!("numstat/port/{}", content.name), |b| {
            b.iter(|| black_box(delta_numstat(&repo, &delta)));
        });

        c.bench_function(&format!("numstat/libgit2-stats/{}", content.name), |b| {
            b.iter(|| {
                let stats = diff.stats().unwrap();
                black_box(stats.insertions() + stats.deletions());
            });
        });
    }
}

criterion_group!(benches, bench_numstat);
criterion_main!(benches);
