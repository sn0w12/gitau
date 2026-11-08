use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git::engines::gix::worktree_icon::find_worktree_icon;
use git2::Repository;

const DIRS: usize = 100;
const FILES_PER_DIR: usize = 100;

/// Ten thousand untracked files, with the only icon at `docs/brand/logo.svg`.
/// Logo is not the best possible rank, so the delegate has to walk the whole
/// worktree before it resolves.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-worktree-icon");
        std::fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        let sig = git2::Signature::now("Bench", "bench@example.com").unwrap();
        std::fs::write(root.join("README.md"), "bench\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("README.md")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        for dir in 0..DIRS {
            let path = root.join(format!("src/d{dir:03}"));
            std::fs::create_dir_all(&path).unwrap();
            for file in 0..FILES_PER_DIR {
                std::fs::write(
                    path.join(format!("f{file:03}.txt")),
                    format!("dir {dir} file {file}\n"),
                )
                .unwrap();
            }
        }

        let brand = root.join("docs/brand");
        std::fs::create_dir_all(&brand).unwrap();
        std::fs::write(
            brand.join("logo.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"/>"#,
        )
        .unwrap();

        std::mem::forget(outer);
        root
    })
}

fn bench_worktree_icon(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let found = find_worktree_icon(&root).unwrap().unwrap();
    assert_eq!(found.content_type, "image/svg+xml");

    let mut group = c.benchmark_group("worktree-icon");
    group.sample_size(10);
    group.bench_function("dirwalk-10000-files", |b| {
        b.iter(|| {
            let icon = find_worktree_icon(black_box(&root)).unwrap().unwrap();
            assert_eq!(icon.content_type, "image/svg+xml");
            black_box(icon.bytes.len())
        })
    });
    group.finish();
}

criterion_group!(benches, bench_worktree_icon);
criterion_main!(benches);
