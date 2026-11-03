use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use highlight::{LineKind, LineRef, SectionHighlighter};

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

fn addition_lines(text: &str) -> Vec<LineRef<'_>> {
    text.lines()
        .enumerate()
        .map(|(idx, line)| LineRef {
            kind: LineKind::Addition,
            old_lineno: None,
            new_lineno: Some(idx as u32 + 1),
            content: line,
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
        let lines = addition_lines(&text);
        group.bench_function(size, |b| {
            b.iter(|| {
                let mut highlighter =
                    SectionHighlighter::new("corpus.rs", b"", text.as_bytes()).unwrap();
                let spans = highlighter.spans(&lines);
                let count: usize = spans
                    .iter()
                    .filter_map(|spans| spans.as_ref())
                    .map(|spans| spans.len() / 3)
                    .sum();
                assert!(count > 0, "fixture must highlight");
                black_box(count)
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_full_file);
criterion_main!(benches);
