use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};

use git_backend::engines::gix::language_stats::language_stats;

/// A few thousand files across common languages plus ignored noise and
/// content-ambiguous files, sized so the walk plus classification is the
/// dominant cost, not process startup.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-langs");
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);
        std::fs::write(root.join(".gitignore"), "node_modules/\ntarget/\n").unwrap();

        let mut files: Vec<(String, String)> = Vec::new();
        for i in 0..1500u32 {
            files.push((
                format!("src/app{i:04}.ts"),
                (0..40)
                    .map(|j| format!("export const v{i}_{j} = {j};\n"))
                    .collect(),
            ));
        }
        for i in 0..800u32 {
            files.push((
                format!("crates/mod{i:04}.rs"),
                (0..40)
                    .map(|j| format!("pub fn f{i}_{j}() -> u32 {{ {j} }}\n"))
                    .collect(),
            ));
        }
        for i in 0..400u32 {
            files.push((
                format!("docs/page{i:04}.md"),
                (0..40).map(|j| format!("line {i} {j}\n")).collect(),
            ));
        }
        for i in 0..200u32 {
            files.push((
                format!("config/setting{i:04}.json"),
                (0..40).map(|j| format!("\"k{i}_{j}\": {j},\n")).collect(),
            ));
        }
        for i in 0..150u32 {
            files.push((
                format!("tests/test{i:04}.py"),
                (0..40)
                    .map(|j| format!("def f{i}_{j}():\n    return {j}\n"))
                    .collect(),
            ));
        }
        for i in 0..80u32 {
            files.push((
                format!("include/header{i:04}.h"),
                "#include <stdio.h>\nint main(void) { return 0; }\n".to_string(),
            ));
        }
        for i in 0..40u32 {
            files.push((
                format!("scripts/run{i:04}.sh"),
                "#!/bin/sh\necho hi\n".to_string(),
            ));
        }
        for i in 0..200u32 {
            files.push((
                format!("node_modules/pkg{i:04}.js"),
                "module.exports = 1;\n".to_string(),
            ));
        }
        for i in 0..200u32 {
            files.push((format!("target/artifact{i:04}.bin"), "noise\n".to_string()));
        }

        for (path, content) in &files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
        }
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Bench", "bench@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        std::mem::forget(outer);
        root
    })
}

fn bench_language_stats(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let stats = language_stats(&root).unwrap();
    assert_eq!(
        stats.entries.first().expect("typescript dominant").language,
        "TypeScript"
    );
    assert_eq!(stats.total_percent, 100);

    c.bench_function("language-stats/cold-3370-files", |b| {
        b.iter(|| {
            let stats = language_stats(black_box(&root)).unwrap();
            black_box(stats.entries.len());
        })
    });
}

criterion_group!(benches, bench_language_stats);
criterion_main!(benches);
