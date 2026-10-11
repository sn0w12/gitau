use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::api::queries::DiffRequest;
use git_backend::engines::gix::diff;
use git_backend::engines::gix::highlight;
use git_backend::streaming::model::{DiffRow, DiffRowKind};

fn lcg(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as f64) / ((1u64 << 31) as f64)
    }
}

const NAMES: &[&str] = &[
    "value", "count", "render", "items", "handler", "config", "result", "payload",
];

fn word(next: &mut impl FnMut() -> f64, index: usize) -> String {
    let pick = NAMES[(next() * NAMES.len() as f64) as usize % NAMES.len()];
    format!("{pick}_{index}")
}

fn size_target(size: &str) -> usize {
    match size {
        "small" => 2_000,
        "medium" => 24_000,
        _ => 120_000,
    }
}

/// A rust file of roughly `target` bytes mixing doc comments, block comments,
/// structs, and function bodies, so the grammar has real work to do. The
/// highlighting engine is grammar-agnostic, so one language across the size
/// ladder covers the cost curve.
fn rust_source(seed: u64, target: usize) -> String {
    let mut next = lcg(seed);
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0usize;
    let rendered_len = |lines: &[String]| lines.iter().map(|l| l.len() + 1).sum::<usize>();
    while rendered_len(&lines) < target {
        let name = word(&mut next, i);
        if i % 7 == 3 {
            lines.push("/// A doc comment describing the item below.".into());
            lines.push("#[derive(Debug, Clone)]".into());
            lines.push(format!("pub struct Item{i} {{"));
            lines.push(format!("    pub {name}: u64,"));
            lines.push("    pub label: String,".into());
            lines.push("}".into());
        } else if i % 7 == 6 {
            lines.push(format!("/* block comment {i}"));
            lines.push("   spanning multiple lines".into());
            lines.push("   before the function body */".into());
        } else {
            lines.push(format!(
                "fn {name}(input: &str, limit: usize) -> Option<u64> {{"
            ));
            lines.push("    let mut total: u64 = 0;".into());
            lines.push("    for (i, byte) in input.bytes().enumerate() {".into());
            lines.push(format!("        if i >= limit {{ break; }} // stop {i}"));
            lines.push("        total += u64::from(byte) * 31;".into());
            lines.push("    }".into());
            lines.push(format!("    Some(total % ({} + 7))", i));
            lines.push("}".into());
        }
        lines.push(String::new());
        i += 1;
    }
    lines.join("\n")
}

fn rust_fixture(seed_a: u64, seed_b: u64) -> (String, String) {
    (rust_source(seed_a, 24_000), rust_source(seed_b, 24_000))
}

fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-highlight");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);

        let (base, edited) = rust_fixture(0x1234, 0x4321);
        let contents = [
            ("src/a.rs", base.clone()),
            ("src/b.rs", base),
            ("src/c.rs", edited.clone()),
        ];
        for (path, data) in &contents {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, data).unwrap();
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

        // Worktree keeps `edited` everywhere so a vs-HEAD diff shows changes.
        for (path, _) in &contents {
            std::fs::write(root.join(path), &edited).unwrap();
        }
        std::mem::forget(outer);
        root
    })
}

fn session_for(root: &std::path::Path) -> &'static git_backend::engines::gix::GixSession {
    static S: OnceLock<git_backend::engines::gix::GixSession> = OnceLock::new();
    S.get_or_init(|| git_backend::engines::gix::GixSession::discover(root).unwrap())
}

/// Every line becomes a synthetic addition numbered on the new side only, so
/// attach() parses exactly one full file.
fn full_file_rows(text: &str) -> Vec<DiffRow> {
    text.lines()
        .enumerate()
        .map(|(idx, line)| DiffRow {
            kind: DiffRowKind::Addition,
            old_lineno: None,
            new_lineno: Some(idx as u32 + 1),
            content: line.to_string(),
            raw_hex: None,
            spans: None,
        })
        .collect()
}

fn bench_full_file(c: &mut Criterion) {
    c.bench_function("syntax-set-cold-init", |b| {
        b.iter(|| black_box(two_face::syntax::extra_newlines().syntaxes().len()))
    });

    let mut group = c.benchmark_group("full-file-highlight");
    for size in ["small", "medium", "large"] {
        let text = rust_source(0x5eed, size_target(size));
        group.bench_function(size, |b| {
            b.iter(|| {
                let mut rows = full_file_rows(&text);
                highlight::attach(&mut rows, "corpus.rs", b"", text.as_bytes());
                let spans: usize = rows
                    .iter()
                    .filter_map(|r| r.spans.as_ref())
                    .map(|s| s.len() / 3)
                    .sum();
                assert!(spans > 0, "fixture must highlight");
                black_box(spans)
            })
        });
    }
    group.finish();
}

fn bench_sections(c: &mut Criterion) {
    let root = fixture_repo();
    let session = session_for(root);
    let blobs = diff::new_blob_cache();
    let request = DiffRequest::default();
    let plans = diff::enumerate(session, &request, &blobs).unwrap();

    let plain = diff::materialize(session, &request, &plans[0], &blobs)
        .unwrap()
        .rows;
    let mut highlighted = plain.clone();
    let (base, edited) = rust_fixture(0x1234, 0x4321);
    highlight::attach(
        &mut highlighted,
        "src/a.rs",
        base.as_bytes(),
        edited.as_bytes(),
    );
    let plain_bytes = serde_json::to_vec(&plain).unwrap().len();
    let highlighted_bytes = serde_json::to_vec(&highlighted).unwrap().len();
    eprintln!(
        "[payload] section JSON bytes: plain={plain_bytes} highlighted={highlighted_bytes} ({:.2}x)",
        highlighted_bytes as f64 / plain_bytes.max(1) as f64
    );

    let mut group = c.benchmark_group("section-inline");
    // The progressive pipeline highlights only the first chunk before shipping
    // it; this is the added first-paint latency budget. The highlighter is
    // rebuilt per iteration because each section gets a fresh one in
    // production.
    group.bench_function("first-chunk-highlight-and-serialize", |b| {
        b.iter(|| {
            let mut hl =
                highlight::SectionHighlighter::new("src/a.rs", base.as_bytes(), edited.as_bytes())
                    .unwrap();
            let mut rows = plain.clone();
            let end = 160.min(rows.len());
            hl.highlight_rows(&mut rows[..end]);
            let bytes = serde_json::to_vec(&rows[..end]).unwrap().len();
            black_box(bytes)
        })
    });

    group.bench_function("full-section-batch", |b| {
        b.iter(|| {
            let mut rows = plain.clone();
            highlight::attach(&mut rows, "src/a.rs", base.as_bytes(), edited.as_bytes());
            black_box(rows.len())
        })
    });
    group.finish();
}

criterion_group!(benches, bench_full_file, bench_sections);
criterion_main!(benches);
