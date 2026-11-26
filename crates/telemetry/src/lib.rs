//! Anonymous, opt-in timing and error telemetry.
//!
//! The crate records a tree of spans: the name of each traced function or call,
//! its duration, and attributes developers attach explicitly. It never records
//! arguments, paths, repository contents, environment variables, hostnames, or
//! any stable identifier.
//!
//! # Disabling
//!
//! Instrumentation is compiled out unless the `enabled` feature is on. Without
//! it [`SpanGuard::enter`] and every other entry point return without touching
//! a clock, and [`trace`] passes the annotated item through unchanged.
//!
//! Even with the feature on, nothing is collected until [`init`] is called with
//! [`Config::enabled`] set to `true`. [`set_enabled`] changes the state at any
//! time, including before [`init`]. Two environment variables hard-disable
//! collection regardless of configuration: `TELEMETRY_ENABLED` set to `0`,
//! `false`, `no`, `off`, or `disabled`, and `DO_NOT_TRACK` set to `1`, `true`,
//! `yes`, or `on`.
//!
//! # Settings
//!
//! A settings toggle needs three calls: [`is_available`] to hide the control in
//! builds without the feature, [`is_enabled`] to render the current state, and
//! [`set_enabled`] to change it. [`enable`] and [`disable`] are shorthand.
//!
//! # Async scopes
//!
//! Spans live in a task-local stack. Wrap each async task root in [`scope`] so
//! the guards inside it share one tree:
//!
//! ```
//! # async fn run() {
//! telemetry::scope(async {
//!     // traced work here
//! })
//! .await;
//! # }
//! ```
//!
//! Blocking code uses [`sync_scope`]. Traced calls outside any scope are
//! dropped without allocating.
//!
//! # Errors
//!
//! [`record_error`] stores an error code plus the error's type name. The
//! displayed message is not captured because messages routinely contain paths,
//! URLs, and branch names. [`record_error_detail`] opts into a sanitized
//! message and source chain for a specific call site.

extern crate self as telemetry;

mod config;
mod error;
mod export;
mod model;
mod sanitize;
mod sink;
mod span;

#[cfg(feature = "enabled")]
mod imp;

use std::future::Future;
use std::sync::Arc;

pub use config::{Arch, BuildProfile, Config, CpuTier, EnvironmentContext, InitError, OsFamily};
pub use error::ErrorRecord;
pub use model::{AttrValue, Batch, Span, Trace, SCHEMA_VERSION};
pub use sink::{CallbackSink, FileSink, FlushReport, Sink};
pub use span::{attach_scope, ScopeHandle, SpanGuard};
pub use telemetry_macros::trace;

/// Installs the global collector. Call once during startup, after consent.
///
/// The initial state is [`Config::enabled`] unless [`set_enabled`] was called
/// before this. Repeated calls return [`InitError::AlreadyInitialized`].
/// `sinks` receive batches from [`flush`]; see [`CallbackSink`] and
/// [`FileSink`].
pub fn init(config: Config, sinks: Vec<Arc<dyn Sink>>) -> Result<(), InitError> {
    #[cfg(feature = "enabled")]
    {
        imp::init(config, sinks)
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = (config, sinks);
        Ok(())
    }
}

/// Turns collection on or off.
///
/// Safe to call before [`init`]: the preference is stored and applied when the
/// collector starts. After initialization it takes effect immediately from any
/// thread. Disabling clears buffered traces and errors, so data is not retained
/// after consent is withdrawn.
pub fn set_enabled(enabled: bool) {
    #[cfg(feature = "enabled")]
    {
        imp::set_enabled(enabled);
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = enabled;
    }
}

/// Turns collection on. Shorthand for `set_enabled(true)`.
pub fn enable() {
    set_enabled(true);
}

/// Turns collection off and clears buffered data. Shorthand for
/// `set_enabled(false)`.
pub fn disable() {
    set_enabled(false);
}

/// Whether this build includes telemetry. Use this to hide the settings
/// control in builds without the feature.
#[inline]
pub fn is_available() -> bool {
    cfg!(feature = "enabled")
}

/// Whether spans and errors are currently collected.
#[inline]
pub fn is_enabled() -> bool {
    #[cfg(feature = "enabled")]
    {
        imp::is_enabled()
    }
    #[cfg(not(feature = "enabled"))]
    {
        false
    }
}

/// Establishes a span scope for an async task.
///
/// Nested scopes reuse the enclosing one, so calling this in a child task while
/// already inside a scope does not split the trace.
pub async fn scope<F: Future>(future: F) -> F::Output {
    #[cfg(feature = "enabled")]
    {
        imp::scope(future).await
    }
    #[cfg(not(feature = "enabled"))]
    {
        future.await
    }
}

/// Ends a call-site span after evaluating the call. Used by [`trace`].
#[doc(hidden)]
#[inline]
pub fn finish_call<T>(_guard: SpanGuard, value: T) -> T {
    value
}

/// Ends a call-site span after the awaited future completes. Used by [`trace`].
#[doc(hidden)]
pub async fn finish_await<F: Future>(_guard: SpanGuard, future: F) -> F::Output {
    future.await
}

/// Establishes a span scope for blocking work. Not valid across `.await`.
pub fn sync_scope<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(feature = "enabled")]
    {
        imp::sync_scope(f)
    }
    #[cfg(not(feature = "enabled"))]
    {
        f()
    }
}

/// Establishes a span scope for blocking work that sampling cannot skip.
///
/// Collection must still be enabled. Use for rare, cheap events such as startup
/// where the configured sample rate would leave most occurrences missing. Not
/// valid across `.await`.
pub fn sync_scope_unconditional<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(feature = "enabled")]
    {
        imp::sync_scope_unconditional(f)
    }
    #[cfg(not(feature = "enabled"))]
    {
        f()
    }
}

/// Attaches a value to the innermost active span. See the [`context!`] macro.
pub fn record_attribute(key: &'static str, value: AttrValue) {
    #[cfg(feature = "enabled")]
    {
        imp::record_attribute(key, value);
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = (key, value);
    }
}

/// Records an error code and the error's type name on the innermost span.
///
/// The displayed message is intentionally not stored. Pass a concrete error
/// type, not `&dyn Error`, so the recorded type name is the useful one.
pub fn record_error<E: std::error::Error>(code: &'static str, error: &E) {
    #[cfg(feature = "enabled")]
    {
        imp::record_error(code, error);
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = (code, error);
    }
}

/// Records an error with a sanitized message and source chain.
///
/// Prefer [`record_error`]. The sanitizer redacts URLs, absolute paths, email
/// addresses, and long opaque identifiers, then truncates the result. It is
/// best effort, so do not pass free-form user data here.
pub fn record_error_detail<E: std::error::Error>(code: &'static str, error: &E) {
    #[cfg(feature = "enabled")]
    {
        imp::record_error_detail(code, error);
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = (code, error);
    }
}

/// Records a sanitized message under an error code, for failures that are not
/// `std::error::Error` values.
pub fn record_error_message(code: &'static str, message: &str) {
    #[cfg(feature = "enabled")]
    {
        imp::record_error_message(code, message);
    }
    #[cfg(not(feature = "enabled"))]
    {
        let _ = (code, message);
    }
}

/// Serializes buffered traces and errors to every installed sink.
///
/// Tree conversion happens here, not at capture time. Run it on a background
/// thread if the buffer can be large. Sinks run on the calling thread. A
/// panicking sink is counted and skipped.
pub fn flush() -> FlushReport {
    #[cfg(feature = "enabled")]
    {
        imp::flush()
    }
    #[cfg(not(feature = "enabled"))]
    {
        FlushReport::default()
    }
}

/// Number of completed traces waiting for [`flush`].
pub fn pending_traces() -> usize {
    #[cfg(feature = "enabled")]
    {
        imp::pending_traces()
    }
    #[cfg(not(feature = "enabled"))]
    {
        0
    }
}

/// Number of traces dropped because the buffer was full since startup.
pub fn dropped_traces() -> u64 {
    #[cfg(feature = "enabled")]
    {
        imp::dropped_traces()
    }
    #[cfg(not(feature = "enabled"))]
    {
        0
    }
}

/// Excludes a call from instrumentation without changing its behavior.
///
/// The [`trace`] pass does not descend into macro bodies, so no span is created
/// for the wrapped expression or for any calls nested inside it.
///
/// ```ignore
/// let loaded = telemetry::skip!(decode_thumbnail(&bytes));
/// ```
///
/// Use the `skip = ["..."]` argument on [`trace`] to exclude a name for a whole
/// function instead.
#[macro_export]
macro_rules! skip {
    ($($tokens:tt)*) => ($($tokens)*);
}

/// Attaches key/value context to the innermost active span.
///
/// ```ignore
/// telemetry::context! {
///     "additions" => diffs.additions,
///     "files_changed" => diffs.files.len(),
/// }
/// ```
///
/// Keys are normalized to lowercase `[a-z0-9._-]`. Text values are sanitized
/// and truncated before they are stored.
#[cfg(feature = "enabled")]
#[macro_export]
macro_rules! context {
    ($($key:literal => $value:expr),* $(,)?) => {
        $(
            $crate::record_attribute($key, $crate::AttrValue::from($value));
        )*
    };
}

/// Compiles to nothing when the `enabled` feature is off, including its
/// arguments.
#[cfg(not(feature = "enabled"))]
#[macro_export]
macro_rules! context {
    ($($tt:tt)*) => {
        ()
    };
}
