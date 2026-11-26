# telemetry

Optional, anonymous timing and error telemetry for the Rust side of the app.
Two crates: `telemetry` holds the runtime, `telemetry-macros` holds the
attribute that rewrites call sites. A proc-macro crate cannot export the
runtime, so the split is required.

Nothing is collected unless a build opts in with the `enabled` feature and the
app calls `init` with collection enabled after consent.

## Instrumenting code

```rust
use telemetry::trace;

#[trace]
async fn load_staged_diffs(repo: &Repo) -> Result<Diff, GitError> {
    let staged = fetch_staged(repo).await?;
    let parsed = parse(&staged)?;
    Ok(parsed)
}
```

The attribute does two things:

1. Opens a span named after the function and closes it when the function
   returns, including early returns and `?`.
2. Wraps every call inside the body in a short-lived guard, so
   `fetch_staged(...).await` and `parse(&staged)` become child spans.

Calls inside nested items and `const` blocks are left alone. Calls invoked
inside macro bodies (`format!`, `vec!`, `tracing::info!`) are not seen by the
AST pass and are not instrumented. Standard library calls and a fixed list of
cheap methods (`.clone()`, `.unwrap()`, `.map()`, `.len()`, constructors, and
so on) are skipped.

Arguments control the pass:

```rust
#[trace(name = "diff.stream", skip = ["render_hunk"], span_only)]
fn stream(...) { ... }
```

- `name = "..."` renames the root span.
- `skip = ["fn_name"]` leaves matching call names uninstrumented.
- `span_only` keeps the function span and skips all call-site guards.

For one call, wrap it in `telemetry::skip!` instead:

```rust
let thumbnail = telemetry::skip!(decode_thumbnail(&bytes));
```

The wrapped expression and every call nested inside it stay out of the trace.

## Performance

The cost depends on which switch is off:

| State                   | Cost                                                                                                                                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Feature off             | Zero. The attribute is a pass-through and every entry point is a no-op.                                                                          |
| Feature on, runtime off | One relaxed atomic load per instrumented call site. No clock reads, no allocation.                                                               |
| Feature on, enabled     | Per span: a state lookup, two clock reads, one spinlock pair, and an arena push and pop. Child links are rebuilt in `flush`, not during capture. |

The benchmark in `benches/overhead.rs` covers primitive costs, a flat call
loop, a nested CPU-bound load with 137 spans and attributes, an async load with
await points, a 16-trace flush, and the runtime-off path:

```bash
cargo bench -p telemetry --bench overhead --features enabled
```

Measured on a Windows desktop with a 1 s warmup and a 3 s measurement:

| Case                          | Time    |
| ----------------------------- | ------- |
| `std::time::Instant::now`     | 20.4 ns |
| `quanta::Instant::now`        | 8.5 ns  |
| `std::time::Instant::elapsed` | 22.1 ns |
| `quanta::Instant::elapsed`    | 10.5 ns |
| Mutex lock and unlock         | 8.2 ns  |
| Arc clone and drop            | 8.0 ns  |
| Empty sync scope              | 89 ns   |
| Scope plus one span           | 178 ns  |
| 32 tiny calls                 | 1.72 µs |
| Realistic baseline            | 61.0 µs |
| Realistic instrumented        | 73.0 µs |
| Async baseline                | 25.1 µs |
| Async instrumented            | 29.0 µs |
| Runtime off, realistic        | 60.9 µs |
| Flush, 16 traces              | 0.19 µs |

An enabled span costs about 50 ns in the flat case. The realistic trace adds
12 µs for 137 spans over a 61 µs CPU-bound operation; the extra per-span cost
there comes from 16 attribute records and an arena that grows past its initial
capacity. Real Git operations take milliseconds and usually produce fewer spans
per unit of work, so the relative cost is lower; the per-span number is what
carries over.

Timestamps use `quanta`'s TSC-backed clock when the platform supports it.
Calibration runs on a background thread started when collection is enabled.
Traces opened before calibration finishes use `std::time::Instant`, and each
trace pins one clock source, so a span never mixes start and end clocks.

The runtime-off path adds about 2 ns per call site. Flush converts a span in
about 1.3 ns. The capture path only writes flat arena entries with names,
nanosecond offsets, and parent indices; JSON and waterfall conversion wait for
`flush`.

If a hot loop shows up in a trace, exclude it with `skip` or
`telemetry::skip!` rather than sampling it away. Sampling drops whole scopes,
so it is a blunt tool when only one call is noisy.

## Scopes

Spans live in a task-local stack. A guard outside a scope is a no-op, so async
entry points must open one:

```rust
let result = telemetry::scope(async move {
    load_staged_diffs(&repo).await
}).await;
```

`scope` reuses an enclosing scope, so it is safe to call at every task root.
Blocking code uses `telemetry::sync_scope(|| ...)` and must not cross an
`.await`.

A task-local scope does not follow work into `spawn_blocking`, and calling
`sync_scope` there would start a separate trace. Capture the scope on the
calling task and attach it inside the blocking closure instead:

```rust
let scope = telemetry::ScopeHandle::capture();
let join = tokio::task::spawn_blocking(move || {
    telemetry::attach_scope(scope, || blocking_git_work())
});
```

`capture` returns `None` outside a scope or when collection is off, and
`attach_scope` runs the closure unchanged in that case.

## Errors

```rust
if let Err(error) = fetch_staged(repo).await {
    telemetry::record_error("diff.fetch_failed", &error);
    return Err(error);
}
```

`record_error` stores the code and the error's Rust type name. It does not call
`Display`, because messages routinely contain paths, URLs, branch names, and
tokens.

For a specific call site that needs more signal, `record_error_detail` also
stores a sanitized message and up to three source errors, and
`record_error_message` handles failures that are not `Error` values. The
sanitizer redacts URLs, absolute paths, emails, and long opaque identifiers,
then truncates. It is best effort: never pass user-authored text.

An error attaches to the innermost active span. Errors recorded with no span
active go into the batch-level `errors` list.

## Context

```rust
telemetry::context! {
    "additions" => diff.additions,
    "deletions" => diff.deletions,
    "files_changed" => diff.files.len(),
}
```

Keys are normalized to lowercase `[a-z0-9._-]`. Text values are sanitized and
truncated to 240 characters. Numeric and boolean values pass through. This is
the only way to make a trace carry more than symbol names and timings, so keep
the values structural and never attach user data.

## Global context

`EnvironmentContext::detect(app_version)` reads the target OS, target
architecture, debug/release profile, and a CPU tier bucket (1 to 2, 3 to 4, 5
to 8, 9 to 16, more than 16 logical cores). It reads no hardware serials,
hostnames, usernames, locale, GPU, or memory size. The context is attached to
every batch.

## Kill switches

| Switch                                          | Effect                                                                                                                           |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `enabled` feature off                           | Attribute is a pass-through; runtime entry points are no-ops; no clock reads                                                     |
| `Config::enabled(false)`                        | No scopes are created                                                                                                            |
| `telemetry::set_enabled(bool)`                  | Works at any time; before `init` it stores the preference, after `init` it applies immediately and clears buffers when disabling |
| `telemetry::enable()` / `disable()`             | Shorthand for `set_enabled`                                                                                                      |
| `TELEMETRY_ENABLED=0\|false\|no\|off\|disabled` | Hard-disables collection for the process                                                                                         |
| `DO_NOT_TRACK=1\|true\|yes\|on`                 | Hard-disables collection for the process                                                                                         |
| `Config::env_kill_switch(false)`                | Ignores the environment variables                                                                                                |

For the settings UI:

```rust
if telemetry::is_available() {
    let enabled = telemetry::is_enabled();
    // render a switch with `enabled`
    // on change:
    telemetry::set_enabled(next);
}
```

`sample_permille` reduces volume by tracing only a fraction of task scopes.
`min_trace_duration_us` drops short traces. Buffer, span, depth, and attribute
caps bound memory; drops are counted in `dropped_traces()` and in the next
batch.

## Output

Call `flush()` periodically and hand the `Batch` to a `Sink`:

```rust
let sink = telemetry::CallbackSink::new(|batch| {
    let body = batch.to_json();
    // enqueue for upload
});
telemetry::init(config, vec![std::sync::Arc::new(sink)]).unwrap();
```

`to_json` produces the upload shape. `to_chrome_trace_json` produces Chrome
Trace Event JSON, which Perfetto (`ui.perfetto.dev`) and `chrome://tracing`
render as a nested waterfall. `FileSink` appends JSON lines locally for
debugging and stays on disk.

Sinks run on the flushing thread. They must not block; enqueue the batch if
delivery can stall.

## What is collected

- Span names taken from Rust identifiers, start offsets, and durations
- Attributes explicitly attached with `context!`
- Error codes and error type names
- Sanitized error detail only where `record_error_detail` or
  `record_error_message` is used
- OS family, architecture, CPU tier bucket, app version, debug/release

Not collected: arguments, return values, file paths, repository or branch
names, diffs, commit hashes, environment variables, usernames, hostnames, IP
addresses, event timestamps, install identifiers, session identifiers, or any
other stable value.

## EU privacy notes

The payload is designed so it cannot single out a person and cannot link two
sessions. With the default error API there are no stable or quasi-stable
identifiers, so the upload is anonymous data rather than personal data. Keep
it that way:

- Do not add identifiers through `context!`. Doing so turns the payload into
  personal data, and the deployer becomes the controller. It then needs a
  lawful basis, a retention limit, and a way to honor access and erasure
  requests.
- The receiving server must not log or store source IP addresses. An IP is
  personal data under GDPR even when the body is anonymous. Terminate TLS
  without address logging, or truncate the address before any log write.
- Keep retention short. Timings are only useful for a few weeks.
- Do not enable collection for an app version before the consent screen
  exists. The crate defaults to disabled.
- The optional `FileSink` writes to the user's disk. It is a debugging tool,
  not a production sink.

## Example

```bash
cargo run -p telemetry --example waterfall --features enabled
```

The example runs a fake repository load with random timings, records an error
with a path in the message, prints the span tree, and writes two files under
the system temp directory: `batch.json` and `trace.json`. Open `trace.json` at
`ui.perfetto.dev` for the waterfall.

## Tests

```bash
cargo test -p telemetry -p telemetry-macros
cargo test -p telemetry -p telemetry-macros --features enabled
cargo bench -p telemetry --bench overhead --features enabled
```

The second command exercises the AST pass and a full trace round trip. The
benchmark prints the numbers in the performance table.
