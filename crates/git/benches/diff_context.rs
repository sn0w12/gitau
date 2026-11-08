use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git::domain::ObjectId;
use git::engines::gix::GixSession;
use git::engines::gix::diff::expand_context;
use git2::{Repository, Signature};

const LINES: usize = 20_000;
const PATH: &str = "src/big.rs";
const BASE_SECONDS: i64 = 1_700_000_000;

/// A 20k-line Rust file. Expanding context reads the old blob, splits every
/// line, and builds a highlighter for the section before returning the window.
fn rust_source() -> String {
    let mut out = String::with_capacity(LINES * 96);
    for line in 0..LINES {
        out.push_str(&format!(
            "pub fn item_{line}(input: &str, limit: usize) -> u64 {{ let mut total = {line}; for byte in input.bytes().take(limit) {{ total += u64::from(byte) * 31; }} total }}\n"
        ));
    }
    out
}

fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-diff-context");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);

        std::fs::write(root.join(PATH), rust_source()).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(PATH)).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::new(
            "Bench",
            "bench@example.com",
            &git2::Time::new(BASE_SECONDS, 0),
        )
        .unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        std::mem::forget(outer);
        root
    })
}

fn old_blob(root: &Path) -> ObjectId {
    let repo = Repository::open(root).unwrap();
    let tree = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let entry = tree.get_path(Path::new(PATH)).unwrap();
    ObjectId::from_bytes(entry.id().as_bytes()).unwrap()
}

fn bench_diff_context(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let session = GixSession::discover(&root).unwrap();
    let old_id = old_blob(&root);

    let mut group = c.benchmark_group("diff-context/20k-line-rust");
    group.sample_size(20);

    for (label, start, count) in [
        ("head-100-rows", 1u32, 100u32),
        ("middle-500-rows", 10_000, 500),
        ("tail-500-rows", 19_500, 500),
    ] {
        group.bench_function(label, |b| {
            b.iter(|| {
                let expansion = expand_context(
                    &session,
                    PATH,
                    Some(old_id),
                    black_box(start),
                    black_box(start),
                    count,
                )
                .unwrap();
                assert_eq!(expansion.rows.len(), count as usize);
                black_box(expansion.rows.len())
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_diff_context);
criterion_main!(benches);
