use std::cell::{RefCell, UnsafeCell};
use std::collections::VecDeque;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::config::{Config, EnvironmentContext, InitError};
use crate::error::ErrorRecord;
use crate::model::{unix_ms_now, AttrValue, Batch, Span, Trace, SCHEMA_VERSION};
use crate::sanitize;
use crate::sink::{FlushReport, Sink};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static SCOPE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static DESIRED_ENABLED: AtomicU8 = AtomicU8::new(CONSENT_UNSET);
static FAST_CLOCK_READY: AtomicBool = AtomicBool::new(false);
static FAST_CLOCK_STARTED: AtomicBool = AtomicBool::new(false);

const CONSENT_UNSET: u8 = 0;
const CONSENT_ON: u8 = 1;
const CONSENT_OFF: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClockSource {
    Std,
    Quanta,
}

impl ClockSource {
    fn current() -> Self {
        if FAST_CLOCK_READY.load(Ordering::Relaxed) {
            Self::Quanta
        } else {
            Self::Std
        }
    }
}

#[derive(Clone, Copy)]
enum ClockInstant {
    Std(std::time::Instant),
    Quanta(quanta::Instant),
}

struct SpinLock<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
}

// SAFETY: the only access to `value` is through a guard that holds `locked`,
// and the guard is released before the borrow ends.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> SpinGuard<'_, T> {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard holds the lock until it is dropped.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard holds the lock until it is dropped.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

impl ClockInstant {
    fn now(source: ClockSource) -> Self {
        match source {
            ClockSource::Std => Self::Std(std::time::Instant::now()),
            ClockSource::Quanta => Self::Quanta(quanta::Instant::now()),
        }
    }

    fn elapsed(self) -> u64 {
        match self {
            Self::Std(instant) => instant.elapsed().as_nanos() as u64,
            Self::Quanta(instant) => instant.elapsed().as_nanos() as u64,
        }
    }

    fn duration_since(self, earlier: Self) -> u64 {
        match (self, earlier) {
            (Self::Std(later), Self::Std(earlier)) => {
                later.duration_since(earlier).as_nanos() as u64
            }
            (Self::Quanta(later), Self::Quanta(earlier)) => {
                later.duration_since(earlier).as_nanos() as u64
            }
            _ => 0,
        }
    }
}

struct Runtime {
    context: EnvironmentContext,
    enabled: AtomicBool,
    env_disabled: bool,
    sample_permille: u16,
    buffer_capacity: usize,
    error_capacity: usize,
    min_trace_duration_us: u64,
    max_spans_per_trace: usize,
    max_depth: usize,
    max_attributes_per_span: usize,
    traces: Mutex<VecDeque<FinishedTrace>>,
    errors: Mutex<VecDeque<ErrorRecord>>,
    dropped_total: AtomicU64,
    dropped_since_flush: AtomicU64,
    sinks: Vec<Arc<dyn Sink>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpanKind {
    Call,
    Function,
}

struct SpanNode {
    name: &'static str,
    kind: SpanKind,
    started_ns: u64,
    duration_ns: u64,
    parent: Option<usize>,
    // Keeps the common empty-attribute case to one pointer; the arena copies
    // nodes when it grows. Keys stay as static literals and are normalized in
    // `flush`.
    #[allow(clippy::box_collection)]
    attributes: Option<Box<Vec<(&'static str, AttrValue)>>>,
    error: Option<Box<ErrorRecord>>,
}

struct FinishedTrace {
    start_unix_ms: u64,
    duration_us: u64,
    dropped_spans: u64,
    root: usize,
    arena: Vec<SpanNode>,
}

impl FinishedTrace {
    fn to_trace(&self) -> Trace {
        let mut children = vec![Vec::new(); self.arena.len()];
        for (index, node) in self.arena.iter().enumerate() {
            if let Some(parent) = node.parent {
                children[parent].push(index);
            }
        }

        Trace {
            start_unix_ms: self.start_unix_ms,
            duration_us: self.duration_us,
            dropped_spans: self.dropped_spans,
            root: span_from_arena(self, &children, self.root, self.arena[self.root].started_ns),
        }
    }
}

struct TraceState {
    clock: ClockSource,
    epoch: ClockInstant,
    arena: Vec<SpanNode>,
    stack: Vec<usize>,
    root: Option<usize>,
    dropped: u64,
    max_spans: usize,
    max_depth: usize,
    max_attributes: usize,
}

impl TraceState {
    fn new(max_spans: usize, max_depth: usize, max_attributes: usize) -> Self {
        let clock = ClockSource::current();
        Self {
            clock,
            epoch: ClockInstant::now(clock),
            arena: Vec::with_capacity(64),
            stack: Vec::with_capacity(16),
            root: None,
            dropped: 0,
            max_spans,
            max_depth,
            max_attributes,
        }
    }
}

tokio::task_local! {
    static TASK_STATE: Arc<SpinLock<TraceState>>;
}

thread_local! {
    static THREAD_STATE: RefCell<Option<Arc<SpinLock<TraceState>>>> = const { RefCell::new(None) };
}

pub(crate) fn init(config: Config, sinks: Vec<Arc<dyn Sink>>) -> Result<(), InitError> {
    let env_disabled = config.env_kill_switch && env_kill_switch_engaged();
    let desired = match DESIRED_ENABLED.load(Ordering::Relaxed) {
        CONSENT_ON => Some(true),
        CONSENT_OFF => Some(false),
        _ => None,
    };
    let enabled = desired.unwrap_or(config.enabled) && !env_disabled;

    let runtime = Runtime {
        context: config.context,
        enabled: AtomicBool::new(enabled),
        env_disabled,
        sample_permille: config.sample_permille,
        buffer_capacity: config.buffer_capacity,
        error_capacity: config.buffer_capacity,
        min_trace_duration_us: config.min_trace_duration_us,
        max_spans_per_trace: config.max_spans_per_trace,
        max_depth: config.max_depth,
        max_attributes_per_span: config.max_attributes_per_span,
        traces: Mutex::new(VecDeque::new()),
        errors: Mutex::new(VecDeque::new()),
        dropped_total: AtomicU64::new(0),
        dropped_since_flush: AtomicU64::new(0),
        sinks,
    };

    RUNTIME
        .set(runtime)
        .map_err(|_| InitError::AlreadyInitialized)?;
    ENABLED.store(enabled, Ordering::Release);
    if enabled {
        start_fast_clock();
    }
    Ok(())
}

fn start_fast_clock() {
    if FAST_CLOCK_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("telemetry-clock".into())
        .spawn(|| {
            let _ = quanta::Instant::now();
            FAST_CLOCK_READY.store(true, Ordering::Release);
        });
}

#[inline]
pub(crate) fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn set_enabled(enabled: bool) {
    DESIRED_ENABLED.store(
        if enabled { CONSENT_ON } else { CONSENT_OFF },
        Ordering::Relaxed,
    );

    let Some(runtime) = RUNTIME.get() else {
        if enabled {
            start_fast_clock();
        }
        return;
    };
    let enabled = enabled && !runtime.env_disabled;
    runtime.enabled.store(enabled, Ordering::Relaxed);
    ENABLED.store(enabled, Ordering::Release);
    if enabled {
        start_fast_clock();
    }

    if !enabled {
        lock(&runtime.traces).clear();
        lock(&runtime.errors).clear();
        runtime.dropped_since_flush.store(0, Ordering::Relaxed);
    }
}

pub(crate) async fn scope<F: Future>(future: F) -> F::Output {
    if !is_enabled() || current_state().is_some() || !sample() {
        return future.await;
    }
    let Some(runtime) = RUNTIME.get() else {
        return future.await;
    };

    let state = Arc::new(SpinLock::new(TraceState::new(
        runtime.max_spans_per_trace,
        runtime.max_depth,
        runtime.max_attributes_per_span,
    )));
    TASK_STATE.scope(state, future).await
}

pub(crate) fn sync_scope<R>(f: impl FnOnce() -> R) -> R {
    sync_scope_inner(f, true)
}

pub(crate) fn sync_scope_unconditional<R>(f: impl FnOnce() -> R) -> R {
    sync_scope_inner(f, false)
}

fn sync_scope_inner<R>(f: impl FnOnce() -> R, sampled: bool) -> R {
    if !is_enabled() || current_state().is_some() || (sampled && !sample()) {
        return f();
    }
    let Some(runtime) = RUNTIME.get() else {
        return f();
    };

    let state = Arc::new(SpinLock::new(TraceState::new(
        runtime.max_spans_per_trace,
        runtime.max_depth,
        runtime.max_attributes_per_span,
    )));
    let previous = THREAD_STATE.with(|slot| slot.replace(Some(state)));
    let _restore = ThreadStateRestore(previous);
    f()
}

#[derive(Clone)]
pub(crate) struct ScopeHandle(Arc<SpinLock<TraceState>>);

pub(crate) fn capture_scope() -> Option<ScopeHandle> {
    if !is_enabled() {
        return None;
    }
    current_state().map(ScopeHandle)
}

pub(crate) fn attach_scope<R>(handle: &ScopeHandle, f: impl FnOnce() -> R) -> R {
    let previous = THREAD_STATE.with(|slot| slot.replace(Some(Arc::clone(&handle.0))));
    let _restore = ThreadStateRestore(previous);
    f()
}

struct ThreadStateRestore(Option<Arc<SpinLock<TraceState>>>);

impl Drop for ThreadStateRestore {
    fn drop(&mut self) {
        THREAD_STATE.with(|slot| {
            *slot.borrow_mut() = self.0.take();
        });
    }
}

pub(crate) fn record_attribute(key: &'static str, value: AttrValue) {
    if !is_enabled() {
        return;
    }
    let Some(state) = current_state() else {
        return;
    };

    let mut state = state.lock();
    let Some(&index) = state.stack.last() else {
        return;
    };
    let max = state.max_attributes;
    if let Some(node) = state.arena.get_mut(index) {
        insert_attribute(node, key, value, max);
    }
}

pub(crate) fn record_error<E: std::error::Error>(code: &'static str, error: &E) {
    if !is_enabled() {
        return;
    }
    attach_or_queue(ErrorRecord::bare(code, error));
}

pub(crate) fn record_error_detail<E: std::error::Error>(code: &'static str, error: &E) {
    if !is_enabled() {
        return;
    }
    attach_or_queue(ErrorRecord::detailed(code, error));
}

pub(crate) fn record_error_message(code: &'static str, message: &str) {
    if !is_enabled() {
        return;
    }
    attach_or_queue(ErrorRecord::from_message(code, message));
}

pub(crate) fn flush() -> FlushReport {
    let Some(runtime) = RUNTIME.get() else {
        return FlushReport::default();
    };
    if !is_enabled() {
        return FlushReport::default();
    }

    let finished: Vec<FinishedTrace> = lock(&runtime.traces).drain(..).collect();
    let errors: Vec<ErrorRecord> = lock(&runtime.errors).drain(..).collect();
    let dropped = runtime.dropped_since_flush.swap(0, Ordering::Relaxed);

    if finished.is_empty() && errors.is_empty() {
        return FlushReport {
            dropped_traces: dropped,
            ..FlushReport::default()
        };
    }

    let report = FlushReport {
        traces: finished.len(),
        errors: errors.len(),
        dropped_traces: dropped,
        sink_failures: 0,
    };

    let traces: Vec<Trace> = finished.iter().map(FinishedTrace::to_trace).collect();
    let batch = Batch {
        schema_version: SCHEMA_VERSION,
        created_at_unix_ms: unix_ms_now(),
        context: runtime.context.clone(),
        traces,
        errors,
        dropped_traces: dropped,
    };

    let mut failures = 0;
    for sink in &runtime.sinks {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sink.accept(&batch);
        }));
        if result.is_err() {
            failures += 1;
        }
    }

    FlushReport {
        sink_failures: failures,
        ..report
    }
}

pub(crate) fn pending_traces() -> usize {
    RUNTIME
        .get()
        .map(|runtime| lock(&runtime.traces).len())
        .unwrap_or(0)
}

pub(crate) fn dropped_traces() -> u64 {
    RUNTIME
        .get()
        .map(|runtime| runtime.dropped_total.load(Ordering::Relaxed))
        .unwrap_or(0)
}

pub(crate) struct Guard {
    state: Arc<SpinLock<TraceState>>,
    index: usize,
    started: ClockInstant,
}

impl Guard {
    pub(crate) fn set_attribute(&self, key: &'static str, value: AttrValue) {
        let mut state = self.state.lock();
        let max = state.max_attributes;
        if let Some(node) = state.arena.get_mut(self.index) {
            insert_attribute(node, key, value, max);
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let duration_ns = self.started.elapsed();

        let mut state = self.state.lock();
        let Some(node) = state.arena.get_mut(self.index) else {
            return;
        };
        node.duration_ns = duration_ns;

        if state.stack.last() == Some(&self.index) {
            state.stack.pop();
        } else {
            let Some(position) = state.stack.iter().rposition(|&index| index == self.index) else {
                return;
            };
            state.stack.remove(position);
        }
        if !state.stack.is_empty() {
            return;
        }

        let Some(root) = state.root.take() else {
            return;
        };
        let duration_us = duration_ns / 1000;
        let finished = FinishedTrace {
            start_unix_ms: unix_ms_now().saturating_sub(duration_us / 1000),
            duration_us,
            dropped_spans: state.dropped,
            root,
            arena: std::mem::take(&mut state.arena),
        };
        state.stack.clear();
        state.dropped = 0;

        drop(state);
        submit(finished);
    }
}

#[inline]
pub(crate) fn enter(name: &'static str) -> Option<Guard> {
    if !is_enabled() {
        return None;
    }
    enter_span(name, SpanKind::Call)
}

#[inline]
pub(crate) fn enter_function(name: &'static str) -> Option<Guard> {
    if !is_enabled() {
        return None;
    }
    enter_span(name, SpanKind::Function)
}

fn enter_span(name: &'static str, kind: SpanKind) -> Option<Guard> {
    let state = current_state()?;

    let (index, started) = {
        let mut trace_state = state.lock();
        if kind == SpanKind::Function {
            if let Some(&top) = trace_state.stack.last() {
                let node = &trace_state.arena[top];
                if node.kind == SpanKind::Call && node.name == name {
                    return None;
                }
            }
        }
        if trace_state.stack.len() >= trace_state.max_depth
            || trace_state.arena.len() >= trace_state.max_spans
        {
            trace_state.dropped += 1;
            return None;
        }

        let now = ClockInstant::now(trace_state.clock);
        let started_ns = now.duration_since(trace_state.epoch);
        let parent = trace_state.stack.last().copied();
        let index = trace_state.arena.len();
        if parent.is_none() {
            trace_state.root = Some(index);
        }
        trace_state.arena.push(SpanNode {
            name,
            kind,
            started_ns,
            duration_ns: 0,
            parent,
            attributes: None,
            error: None,
        });
        trace_state.stack.push(index);
        (index, now)
    };

    Some(Guard {
        state,
        index,
        started,
    })
}

fn current_state() -> Option<Arc<SpinLock<TraceState>>> {
    if let Some(state) = THREAD_STATE.with(|slot| slot.borrow().clone()) {
        return Some(state);
    }
    TASK_STATE.try_with(Arc::clone).ok()
}

fn submit(trace: FinishedTrace) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    if !runtime.enabled.load(Ordering::Relaxed) {
        return;
    }
    if trace.duration_us < runtime.min_trace_duration_us {
        return;
    }

    let mut traces = lock(&runtime.traces);
    if traces.len() >= runtime.buffer_capacity {
        traces.pop_front();
        runtime.dropped_total.fetch_add(1, Ordering::Relaxed);
        runtime.dropped_since_flush.fetch_add(1, Ordering::Relaxed);
    }
    traces.push_back(trace);
}

fn queue_error(record: ErrorRecord) {
    let Some(runtime) = RUNTIME.get() else {
        return;
    };
    if !runtime.enabled.load(Ordering::Relaxed) {
        return;
    }

    let mut errors = lock(&runtime.errors);
    if errors.len() >= runtime.error_capacity {
        errors.pop_front();
    }
    errors.push_back(record);
}

fn attach_or_queue(mut record: ErrorRecord) {
    if let Some(state) = current_state() {
        let mut state = state.lock();
        if let Some(&index) = state.stack.last() {
            if let Some(node) = state.arena.get_mut(index) {
                if node.error.is_none() {
                    node.error = Some(Box::new(record));
                    return;
                }
            }
        }
    }

    record.at_unix_ms = Some(unix_ms_now());
    queue_error(record);
}

fn span_from_arena(
    trace: &FinishedTrace,
    children: &[Vec<usize>],
    index: usize,
    base_ns: u64,
) -> Span {
    let node = &trace.arena[index];
    Span {
        name: node.name.to_string(),
        start_us: node.started_ns.saturating_sub(base_ns) / 1000,
        duration_us: node.duration_ns / 1000,
        attributes: node
            .attributes
            .as_deref()
            .map(|entries| {
                entries
                    .iter()
                    .filter_map(|(key, value)| sanitize::key(key).map(|key| (key, value.clone())))
                    .collect()
            })
            .unwrap_or_default(),
        error: node.error.as_deref().cloned(),
        children: children[index]
            .iter()
            .map(|&child| span_from_arena(trace, children, child, base_ns))
            .collect(),
    }
}

fn insert_attribute(
    node: &mut SpanNode,
    key: &'static str,
    value: AttrValue,
    max_attributes: usize,
) {
    match &mut node.attributes {
        Some(entries) => {
            if let Some(slot) = entries.iter_mut().find(|(existing, _)| *existing == key) {
                slot.1 = value;
            } else if entries.len() < max_attributes {
                entries.push((key, value));
            }
        }
        None if max_attributes > 0 => {
            node.attributes = Some(Box::new(vec![(key, value)]));
        }
        None => {}
    }
}

fn sample() -> bool {
    let Some(runtime) = RUNTIME.get() else {
        return false;
    };
    let permille = runtime.sample_permille;
    if permille >= 1000 {
        return true;
    }
    if permille == 0 {
        return false;
    }
    let sequence = SCOPE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    sequence % 1000 < u64::from(permille)
}

fn env_kill_switch_engaged() -> bool {
    let telemetry_disabled = std::env::var("TELEMETRY_ENABLED")
        .map(|value| is_falsy_flag(&value))
        .unwrap_or(false);
    let do_not_track = std::env::var("DO_NOT_TRACK")
        .map(|value| is_truthy_flag(&value))
        .unwrap_or(false);
    telemetry_disabled || do_not_track
}

fn is_falsy_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off" | "disabled"
    )
}

fn is_truthy_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_do_not_track_flags() {
        assert!(is_truthy_flag("1"));
        assert!(is_truthy_flag(" true "));
        assert!(is_truthy_flag("YES"));
        assert!(!is_truthy_flag("0"));
        assert!(!is_truthy_flag("false"));
        assert!(!is_truthy_flag(""));
    }

    #[test]
    fn recognizes_telemetry_disabled_flags() {
        assert!(is_falsy_flag("0"));
        assert!(is_falsy_flag(" false "));
        assert!(is_falsy_flag("OFF"));
        assert!(!is_falsy_flag("1"));
        assert!(!is_falsy_flag("true"));
        assert!(!is_falsy_flag(""));
    }
}
