use std::hint::black_box;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::api::queries::DiffRequest;
use git_backend::domain::{Generation, OperationId, RepoId, SnapshotId};
use git_backend::engines::gix::diff;
use git_backend::engines::gix::highlight;
use git_backend::streaming::model::{DiffEvent, DiffRow, DiffRowKind};
use git_backend::streaming::pipeline::{self, DiffJob};
use git_backend::streaming::store::DiffOperation;

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

struct Rng {
    next: Box<dyn FnMut() -> f64>,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            next: Box::new(lcg(seed)),
        }
    }
    fn word(&mut self, index: usize) -> String {
        let pick = NAMES[((self.next)() * NAMES.len() as f64) as usize % NAMES.len()];
        format!("{pick}_{index}")
    }
}

fn size_target(size: &str) -> usize {
    match size {
        "small" => 2_000,
        "medium" => 24_000,
        _ => 120_000,
    }
}

fn source(seed: u64, lang: &str, target: usize) -> String {
    let mut rng = Rng::new(seed);
    let mut lines: Vec<String> = Vec::new();
    let mut i = 0usize;
    let rendered_len = |lines: &[String]| lines.iter().map(|l| l.len() + 1).sum::<usize>();
    while rendered_len(&lines) < target {
        match lang {
            "rust" => {
                let name = rng.word(i);
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
            }
            "tsx" => {
                let name = rng.word(i);
                lines.push(format!("interface Item{i}Props {{"));
                lines.push(format!("    {name}: number;"));
                lines.push("    onDone?: () => void;".into());
                lines.push("}".into());
                lines.push(String::new());
                lines.push(format!("export function Item{i}(props: Item{i}Props) {{"));
                lines.push("    const [ready, setReady] = React.useState(false);".into());
                lines.push("    React.useEffect(() => {".into());
                lines.push("        setReady(true);".into());
                lines.push("    }, []);".into());
                lines.push("    return (".into());
                lines.push("        <div className=\"panel\">".into());
                lines.push(format!(
                    "            <span data-ready={{ready}}>{{props.{name}}} items</span>"
                ));
                lines.push("        </div>".into());
                lines.push("    );".into());
                lines.push("}".into());
            }
            "python" => {
                let name = rng.word(i);
                lines.push(format!("def {name}(items, limit={i}):"));
                lines.push("    \"\"\"Process up to limit items.\"\"\"".into());
                lines.push("    total = 0".into());
                lines.push("    for item in items[:limit]:".into());
                lines.push("        total += hash(item) % 97".into());
                lines.push("    return total".into());
                lines.push(String::new());
                lines.push(format!("class Item{i}:"));
                lines.push(format!("    value = {i}"));
            }
            "go" => {
                lines.push(format!("func Render{i}(xs []int, n int) int {{"));
                lines.push("\ttotal := 0".into());
                lines.push("\tfor i := 0; i < n && i < len(xs); i++ {".into());
                lines.push(format!("\t\ttotal += xs[i] * {}", i + 3));
                lines.push("\t}".into());
                lines.push("\treturn total".into());
                lines.push("}".into());
            }
            "json" => {
                lines.push(format!(
                    "{{ \"id\": {i}, \"active\": {}, \"score\": {:.2}, \"tags\": [\"a\", \"b\"] }}",
                    i % 2 == 0,
                    (rng.next)() * 1000.0
                ));
            }
            "yaml" => {
                lines.push(format!("value_{i}:"));
                lines.push(format!("  id: {i}"));
                lines.push(format!("  name: \"item {i}\""));
                lines.push("  flags:".into());
                lines.push("    - alpha".into());
            }
            "markdown" => {
                lines.push(format!("## Section {i}"));
                lines.push(String::new());
                lines.push("Some prose with `inline code` and **bold** text.".into());
                lines.push("```rust".into());
                lines.push(format!("let x = {i}; // snippet"));
                lines.push("```".into());
            }
            "css" => {
                lines.push(format!(".item-{i} .child {{"));
                lines.push(format!("  color: rgb({}, 16, 32);", i % 255));
                lines.push("  display: flex;".into());
                lines.push("}".into());
            }
            "toml" => {
                lines.push(format!("[section_{i}]"));
                lines.push(format!("name = \"item {i}\""));
                lines.push(format!("enabled = {}", i % 2 == 0));
                lines.push(format!("threshold = {}", i * 13));
            }
            _ => unreachable!(),
        }
        lines.push(String::new());
        i += 1;
    }
    lines.join("\n")
}

const LANGS: &[(&str, &str)] = &[
    ("rust", "rs"),
    ("tsx", "tsx"),
    ("python", "py"),
    ("go", "go"),
    ("json", "json"),
    ("yaml", "yaml"),
    ("markdown", "md"),
    ("css", "css"),
    ("toml", "toml"),
];

fn rust_fixture(seed_a: u64, seed_b: u64) -> (String, String) {
    (
        source(seed_a, "rust", 24_000),
        source(seed_b, "rust", 24_000),
    )
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

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
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
    for (lang_idx, (lang, ext)) in LANGS.iter().enumerate() {
        for size in ["small", "medium", "large"] {
            let text = source(0x5eed ^ ((lang_idx as u64) * 7919), lang, size_target(size));
            let path = format!("corpus.{ext}");
            group.bench_function(format!("{lang}/{size}"), |b| {
                b.iter(|| {
                    let mut rows = full_file_rows(&text);
                    highlight::attach(&mut rows, &path, b"", text.as_bytes());
                    let spans: usize = rows
                        .iter()
                        .filter_map(|r| r.spans.as_ref())
                        .map(|s| s.len() / 3)
                        .sum();
                    black_box(spans)
                })
            });
        }
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
    group.bench_function("materialize-plain", |b| {
        b.iter(|| {
            black_box(
                diff::materialize(session, &request, black_box(&plans[0]), &blobs)
                    .unwrap()
                    .rows,
            )
        })
    });

    // The progressive pipeline highlights only the first chunk before
    // shipping it; this is the added first-paint latency budget.
    group.bench_function("first-chunk-highlight-and-serialize", |b| {
        let mut hl =
            highlight::SectionHighlighter::new("src/a.rs", base.as_bytes(), edited.as_bytes())
                .unwrap();
        b.iter(|| {
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

    c.bench_function("full-stream", |b| {
        b.iter(|| {
            let op = Arc::new(DiffOperation::new(RepoId(7), OperationId(7), Generation(1)));
            let (tx, mut rx) = tokio::sync::mpsc::channel::<DiffEvent>(4096);
            let job = DiffJob::new(
                root.to_path_buf(),
                DiffRequest::default(),
                OperationId(7),
                SnapshotId(0),
                Generation(1),
                session_for(root).clone(),
            );
            let op_for_job = op.clone();
            runtime().block_on(async {
                let handle =
                    tokio::task::spawn_blocking(move || pipeline::run(job, op_for_job, tx));
                while let Some(event) = rx.recv().await {
                    if matches!(event, DiffEvent::Completed { .. }) {
                        break;
                    }
                }
                handle.await.unwrap();
            });
        })
    });
}

criterion_group!(benches, bench_full_file, bench_sections);
criterion_main!(benches);
