#![cfg(feature = "enabled")]

use std::future::Future;
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::Duration;

use telemetry::{AttrValue, Batch, Config, EnvironmentContext, Sink, SpanGuard, Trace};

#[derive(Debug, Default)]
struct Capture {
    batches: Mutex<Vec<Batch>>,
}

impl Sink for Capture {
    fn accept(&self, batch: &Batch) {
        self.batches.lock().unwrap().push(batch.clone());
    }
}

static CAPTURE: OnceLock<Arc<Capture>> = OnceLock::new();
static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        telemetry::set_enabled(false);

        let capture = Arc::new(Capture::default());
        CAPTURE.set(Arc::clone(&capture)).unwrap();

        let config = Config::new(EnvironmentContext::detect("test"))
            .enabled(true)
            .env_kill_switch(false)
            .sample_permille(1000)
            .buffer_capacity(64);
        telemetry::init(config, vec![capture]).unwrap();

        assert!(!telemetry::is_enabled());
        assert!(telemetry::is_available());
        telemetry::enable();
        assert!(telemetry::is_enabled());
    });
}

fn last_batch() -> Batch {
    let batches = CAPTURE.get().unwrap().batches.lock().unwrap();
    batches.last().cloned().expect("a flushed batch")
}

fn trace_named<'a>(batch: &'a Batch, name: &str) -> &'a Trace {
    batch
        .traces
        .iter()
        .find(|trace| trace.root.name == name)
        .unwrap_or_else(|| panic!("trace `{name}` not found"))
}

#[telemetry::trace]
fn outer(value: &str) -> usize {
    let copied = value.to_owned();
    inner(copied.len())
}

fn inner(value: usize) -> usize {
    value + 1
}

#[telemetry::trace]
fn borrows_temporaries() -> usize {
    let text = &make_text();
    let borrowed = take(&make_text());
    text.len() + borrowed
}

fn make_text() -> String {
    String::from("hello")
}

fn take(text: &str) -> usize {
    text.len()
}

const fn array_len() -> usize {
    3
}

fn const_generic<const N: usize>() -> usize {
    N
}

#[telemetry::trace]
fn uses_const_contexts() -> usize {
    let buffer = [0u8; array_len()];
    const_generic::<{ array_len() }>() + buffer.len()
}

#[derive(Default)]
struct Counter {
    value: usize,
}

impl Counter {
    fn bump(&mut self) -> usize {
        self.value += 1;
        self.value
    }
}

#[telemetry::trace]
fn kitchen_sink(input: Option<&str>) -> Result<usize, std::io::Error> {
    let mut counter = Counter::default();
    let mut total = 0usize;

    if let Some(text) = input {
        total += text.len();
    }
    for part in ["a", "b"] {
        if part.is_empty() {
            continue;
        }
        total += part.len();
    }
    while counter.value < 2 {
        total += counter.bump();
    }

    let mapped = input.map(str::len).unwrap_or_default();
    let formatted = format!("{total}-{mapped}");
    let borrowed = &formatted[..1];
    let mut buffer = [0u8; 2];
    buffer[0] = borrowed.len() as u8;

    let result = match input {
        Some(text) => text.len(),
        None => 0,
    };
    let closure = |value: usize| value + result;
    let doubled = closure(total);
    if doubled > 100 {
        return Ok(doubled);
    }

    let _ = buffer;
    Ok(doubled + counter.value)
}

fn skipped_work() -> usize {
    10
}

fn visible_work() -> usize {
    1
}

#[telemetry::trace]
fn skips_one_call() -> usize {
    let hidden = telemetry::skip!(skipped_work());
    let visible = visible_work();
    hidden + visible
}

#[telemetry::trace]
fn outer_traced() -> usize {
    inner_traced()
}

#[telemetry::trace]
fn inner_traced() -> usize {
    1
}

#[telemetry::trace]
async fn outer_async() -> u32 {
    inner_async().await + 1
}

async fn inner_async() -> u32 {
    1
}

#[test]
fn builds_traces_and_reports_errors() {
    setup();

    telemetry::sync_scope(|| {
        let root = SpanGuard::enter("manual_root");
        {
            let _child = SpanGuard::enter("manual_child");
            std::thread::sleep(Duration::from_millis(2));
            telemetry::context! {
                "items" => 3u64,
                "cached" => false,
            };
        }
        telemetry::record_error_message(
            "manual_failure",
            "failed at C:\\Users\\bob\\repo and https://example.com/x",
        );
        drop(root);
    });

    assert_eq!(telemetry::sync_scope(|| outer("abc")), 4);
    assert_eq!(telemetry::sync_scope(borrows_temporaries), 10);
    assert_eq!(telemetry::sync_scope(uses_const_contexts), 6);
    assert_eq!(
        telemetry::sync_scope(|| kitchen_sink(Some("abc"))).unwrap(),
        13
    );
    assert_eq!(telemetry::sync_scope(outer_traced), 1);
    assert_eq!(telemetry::sync_scope(skips_one_call), 11);

    telemetry::sync_scope(|| {
        let root = SpanGuard::enter("attached_root");
        let scope = telemetry::ScopeHandle::capture();
        assert!(scope.is_some(), "captured scope");
        std::thread::spawn(move || {
            telemetry::attach_scope(scope, || {
                let _child = SpanGuard::enter("attached_child");
            });
        })
        .join()
        .unwrap();
        drop(root);
    });

    let mut polled = Box::pin(telemetry::scope(async { 7 }));
    let waker = std::task::Waker::noop();
    let mut poll_context = std::task::Context::from_waker(waker);
    assert!(matches!(
        polled.as_mut().poll(&mut poll_context),
        std::task::Poll::Ready(7)
    ));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let value = runtime.block_on(telemetry::scope(outer_async()));
    assert_eq!(value, 2);

    drop(runtime);

    let report = telemetry::flush();
    assert_eq!(report.sink_failures, 0);
    assert_eq!(report.dropped_traces, 0);
    assert!(!CAPTURE.get().unwrap().batches.lock().unwrap().is_empty());

    let batch = last_batch();
    assert!(batch
        .traces
        .iter()
        .any(|trace| trace.root.name == "manual_root"));

    let manual = trace_named(&batch, "manual_root");
    assert_eq!(manual.root.children.len(), 1);
    let child = &manual.root.children[0];
    assert_eq!(child.name, "manual_child");
    assert_eq!(child.attributes.get("items"), Some(&AttrValue::Uint(3)));
    assert_eq!(
        child.attributes.get("cached"),
        Some(&AttrValue::Bool(false))
    );

    let attached = trace_named(&batch, "attached_root");
    assert_eq!(attached.root.children.len(), 1);
    assert_eq!(attached.root.children[0].name, "attached_child");

    let error = manual.root.error.as_ref().expect("error on root");
    assert_eq!(error.code, "manual_failure");
    assert_eq!(error.message.as_deref(), Some("failed at <path>"));

    let traced = trace_named(&batch, "outer");
    let child_names: Vec<&str> = traced
        .root
        .children
        .iter()
        .map(|span| span.name.as_str())
        .collect();
    assert_eq!(child_names, vec!["inner"]);
    assert!(traced
        .root
        .children
        .iter()
        .all(|span| span.duration_us <= traced.root.duration_us));

    let const_contexts = trace_named(&batch, "uses_const_contexts");
    assert!(const_contexts
        .root
        .children
        .iter()
        .any(|span| span.name == "const_generic"));

    let nested = trace_named(&batch, "outer_traced");
    assert_eq!(nested.root.children.len(), 1);
    assert_eq!(nested.root.children[0].name, "inner_traced");
    assert!(nested.root.children[0].children.is_empty());

    let skipped = trace_named(&batch, "skips_one_call");
    let skipped_children: Vec<&str> = skipped
        .root
        .children
        .iter()
        .map(|span| span.name.as_str())
        .collect();
    assert_eq!(skipped_children, vec!["visible_work"]);

    let borrowed = trace_named(&batch, "borrows_temporaries");
    let borrowed_children: Vec<&str> = borrowed
        .root
        .children
        .iter()
        .map(|span| span.name.as_str())
        .collect();
    assert_eq!(borrowed_children, vec!["make_text", "take"]);
    let take_span = borrowed
        .root
        .children
        .iter()
        .find(|span| span.name == "take")
        .unwrap();
    assert!(take_span
        .children
        .iter()
        .any(|span| span.name == "make_text"));

    let asynchronous = trace_named(&batch, "outer_async");
    let async_children: Vec<&str> = asynchronous
        .root
        .children
        .iter()
        .map(|span| span.name.as_str())
        .collect();
    assert_eq!(async_children, vec!["inner_async"]);

    let chrome: serde_json::Value = serde_json::from_str(&batch.to_chrome_trace_json()).unwrap();
    assert!(chrome["traceEvents"].as_array().unwrap().len() >= 5);

    telemetry::set_enabled(false);
    assert!(!telemetry::is_enabled());
    telemetry::sync_scope(|| {
        let _ = SpanGuard::enter("after_disable");
    });
    assert_eq!(telemetry::pending_traces(), 0);
}
