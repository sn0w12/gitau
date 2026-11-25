//! Runs a fake repository load, prints the collected span tree, and writes a
//! Chrome trace for Perfetto.
//!
//! ```text
//! cargo run -p telemetry --example waterfall --features enabled
//! ```

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use telemetry::{
    trace, AttrValue, Batch, Config, EnvironmentContext, FlushReport, Sink, Span, SpanGuard,
};

struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        self.0 = state;
        state
    }

    fn range(&mut self, min: u64, max: u64) -> u64 {
        min + self.next() % (max - min + 1)
    }

    fn work(&mut self, min_ms: u64, max_ms: u64) {
        std::thread::sleep(Duration::from_millis(self.range(min_ms, max_ms)));
    }
}

#[derive(Clone, Default)]
struct Capture {
    batch: Arc<Mutex<Option<Batch>>>,
}

impl Sink for Capture {
    fn accept(&self, batch: &Batch) {
        *self.batch.lock().unwrap() = Some(batch.clone());
    }
}

#[derive(Debug)]
struct DemoError {
    path: String,
    url: String,
}

impl std::fmt::Display for DemoError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "could not read {} while posting to {}",
            self.path, self.url
        )
    }
}

impl std::error::Error for DemoError {}

#[trace]
async fn read_index(path: &str) -> Result<u64, DemoError> {
    tokio::time::sleep(Duration::from_millis(25)).await;
    if path.is_empty() {
        return Err(DemoError {
            path: path.to_owned(),
            url: "https://example.invalid/v1/telemetry".to_owned(),
        });
    }
    telemetry::context! { "index_entries" => 137u64 };
    Ok(137)
}

#[trace(skip = ["work"])]
fn hash_objects(rng: &mut Rng, count: u64) -> u64 {
    rng.work(6, 18);
    let mut total = 0u64;
    for _ in 0..count {
        total += rng.next() % 4096;
    }
    telemetry::context! { "objects_hashed" => count };
    total
}

#[trace(skip = ["work"])]
fn scan_objects(rng: &mut Rng, entries: u64) -> u64 {
    rng.work(10, 25);
    let objects = entries / 3 + 1;
    let checksum = hash_objects(rng, objects);
    telemetry::context! {
        "objects" => objects,
        "checksum" => checksum % 10_000,
    };
    objects
}

#[trace(skip = ["work"])]
fn parse_packfile(rng: &mut Rng, entries: u64) -> u64 {
    rng.work(8, 20);
    let objects = scan_objects(rng, entries);
    telemetry::context! { "packed_objects" => objects };
    objects
}

#[trace(skip = ["work"])]
fn render_hunk(rng: &mut Rng, index: u64) -> u64 {
    rng.work(2, 9);
    telemetry::context! { "hunk" => index };
    64 + rng.next() % 256
}

#[trace(skip = ["work"])]
fn render_diff(rng: &mut Rng, objects: u64) -> u64 {
    rng.work(8, 20);
    let chunks = objects % 5 + 1;
    let mut bytes = 0u64;
    for chunk in 0..chunks {
        bytes += render_hunk(rng, chunk);
    }
    telemetry::context! {
        "chunks" => chunks,
        "rendered_bytes" => bytes,
    };
    bytes
}

#[trace(skip = ["work"])]
async fn load_repository(path: &str, rng: &mut Rng) -> Result<u64, DemoError> {
    let entries = read_index(path).await?;
    let objects = parse_packfile(rng, entries);
    let rendered = render_diff(rng, objects);

    let error = DemoError {
        path: format!("{path}/.git/objects/pack"),
        url: "https://example.invalid/v1/telemetry".to_owned(),
    };
    telemetry::record_error_detail("object_store.read_failed", &error);

    telemetry::context! {
        "files" => entries,
        "objects" => objects,
        "rendered_bytes" => rendered,
    };

    Ok(rendered)
}

fn main() {
    let capture = Arc::new(Capture::default());
    let config = Config::new(EnvironmentContext::detect(env!("CARGO_PKG_VERSION")))
        .enabled(true)
        .env_kill_switch(false)
        .sample_permille(1000)
        .buffer_capacity(16);

    telemetry::init(config, vec![capture.clone()]).expect("telemetry is not initialized twice");

    if !telemetry::is_enabled() {
        eprintln!("telemetry is disabled for this process");
        return;
    }

    println!("telemetry example: fake repository load\n");

    let mut rng = Rng::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let result = runtime.block_on(telemetry::scope(load_repository(
        "/home/example/repository",
        &mut rng,
    )));

    telemetry::sync_scope(|| {
        let _maintenance = SpanGuard::enter("startup.maintenance");
        std::thread::sleep(Duration::from_millis(12));
        telemetry::context! { "cache_entries" => 42u64 };
    });

    let report = telemetry::flush();
    let batch = capture.batch.lock().unwrap().take();
    let Some(batch) = batch else {
        eprintln!("no batch was flushed");
        return;
    };

    match result {
        Ok(bytes) => println!("work completed: {bytes} rendered bytes\n"),
        Err(error) => println!("work failed: {error}\n"),
    }

    print_batch(&batch, report);

    match write_files(&batch) {
        Ok((batch_path, trace_path)) => {
            println!("\nbatch json: {}", batch_path.display());
            println!("waterfall:  {}", trace_path.display());
            println!("open the waterfall at https://ui.perfetto.dev");
        }
        Err(error) => eprintln!("could not write telemetry files: {error}"),
    }
}

fn print_batch(batch: &Batch, report: FlushReport) {
    let context = serde_json::to_value(&batch.context).expect("context serializes");
    println!(
        "context: os={} arch={} cpu={} build={} app={}",
        string(&context["os"]),
        string(&context["arch"]),
        string(&context["cpuTier"]),
        string(&context["buildProfile"]),
        string(&context["appVersion"]),
    );

    let mut spans = 0;
    let mut errors = batch.errors.len();
    for (index, trace) in batch.traces.iter().enumerate() {
        println!("\ntrace {} ({}):", index + 1, duration(trace.duration_us));
        spans += print_span(&trace.root, 1);
        errors += count_errors(&trace.root);
    }

    for error in &batch.errors {
        println!("\nunattached error: {} ({})", error.code, error.kind);
        if let Some(message) = &error.message {
            println!("  message: {message}");
        }
    }

    let error_word = if errors == 1 { "error" } else { "errors" };
    println!(
        "\nsummary: {} traces, {spans} spans, {errors} {error_word}, {} dropped",
        report.traces, report.dropped_traces
    );
}

fn count_errors(span: &Span) -> usize {
    usize::from(span.error.is_some()) + span.children.iter().map(count_errors).sum::<usize>()
}

fn print_span(span: &Span, depth: usize) -> usize {
    let indent = "  ".repeat(depth);
    let mut line = format!("{indent}{}  {}", span.name, duration(span.duration_us));
    if !span.attributes.is_empty() {
        let attributes = span
            .attributes
            .iter()
            .map(|(key, value)| format!("{key}={}", value_text(value)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(line, "  [{attributes}]");
    }
    println!("{line}");

    if let Some(error) = &span.error {
        let error_indent = "  ".repeat(depth + 1);
        println!("{error_indent}error: {} ({})", error.code, error.kind);
        if let Some(message) = &error.message {
            println!("{error_indent}  message: {message}");
        }
        for source in &error.sources {
            println!("{error_indent}  caused by: {source}");
        }
    }

    1 + span
        .children
        .iter()
        .map(|child| print_span(child, depth + 1))
        .sum::<usize>()
}

fn value_text(value: &AttrValue) -> String {
    match value {
        AttrValue::Bool(value) => value.to_string(),
        AttrValue::Int(value) => value.to_string(),
        AttrValue::Uint(value) => value.to_string(),
        AttrValue::Float(value) => format!("{value:.2}"),
        AttrValue::Text(value) => value.clone(),
    }
}

fn string(value: &serde_json::Value) -> &str {
    value.as_str().unwrap_or("?")
}

fn duration(micros: u64) -> String {
    if micros >= 1000 {
        format!("{:.1} ms", micros as f64 / 1000.0)
    } else {
        format!("{micros} us")
    }
}

fn write_files(batch: &Batch) -> std::io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let directory = std::env::temp_dir().join("gitau-telemetry-example");
    std::fs::create_dir_all(&directory)?;
    let batch_path = directory.join("batch.json");
    let trace_path = directory.join("trace.json");
    std::fs::write(&batch_path, batch.to_json_pretty())?;
    std::fs::write(&trace_path, batch.to_chrome_trace_json())?;
    Ok((batch_path, trace_path))
}
