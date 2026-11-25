use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use telemetry::{trace, Batch, Config, EnvironmentContext, Sink};

struct CountingSink {
    batches: AtomicU64,
}

impl Sink for CountingSink {
    fn accept(&self, _batch: &Batch) {
        self.batches.fetch_add(1, Ordering::Relaxed);
    }
}

#[inline(never)]
fn work(seed: u64, rounds: u64) -> u64 {
    let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for round in 0..rounds {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state = state.wrapping_add(round);
    }
    state
}

#[inline(never)]
fn tiny(seed: u64) -> u64 {
    seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn tiny_baseline(steps: u64) -> u64 {
    let mut total = 0u64;
    for step in 0..steps {
        total = total.wrapping_add(tiny(step));
    }
    total
}

#[trace]
fn tiny_traced(steps: u64) -> u64 {
    let mut total = 0u64;
    for step in 0..steps {
        total = total.wrapping_add(tiny(step));
    }
    total
}

#[trace(span_only)]
fn tiny_span_only(steps: u64) -> u64 {
    let mut total = 0u64;
    for step in 0..steps {
        total = total.wrapping_add(tiny(step));
    }
    total
}

fn tiny_manual(steps: u64) -> u64 {
    telemetry::sync_scope(|| {
        let root = telemetry::SpanGuard::enter("tiny_manual");
        let mut total = 0u64;
        for step in 0..steps {
            let guard = telemetry::SpanGuard::enter("tiny");
            total = total.wrapping_add(tiny(step));
            drop(guard);
        }
        drop(root);
        total
    })
}

fn flat_baseline(steps: u64) -> u64 {
    let mut total = 0u64;
    for step in 0..steps {
        total = total.wrapping_add(work(step, 32));
    }
    total
}

#[trace]
fn flat_traced(steps: u64) -> u64 {
    let mut total = 0u64;
    for step in 0..steps {
        total = total.wrapping_add(work(step, 32));
    }
    total
}

#[inline(never)]
fn read_index_plain(seed: u64) -> u64 {
    work(seed, 1000)
}

#[inline(never)]
fn scan_objects_plain(seed: u64) -> u64 {
    work(seed ^ 0x5bf0_3635, 800)
}

#[inline(never)]
fn parse_pack_plain(seed: u64) -> u64 {
    scan_objects_plain(seed)
        .wrapping_add(scan_objects_plain(seed.rotate_left(7)))
        .wrapping_add(work(seed, 600))
}

#[inline(never)]
fn render_hunk_plain(seed: u64, hunk: u64) -> u64 {
    work(seed ^ hunk.wrapping_mul(31), 500)
}

#[inline(never)]
fn scan_commit_plain(seed: u64) -> u64 {
    let mut total = read_index_plain(seed);
    total = total.wrapping_add(parse_pack_plain(seed));
    for hunk in 0..4 {
        total = total.wrapping_add(render_hunk_plain(seed, hunk));
    }
    total
}

#[inline(never)]
fn realistic_baseline(commits: u64) -> u64 {
    let mut total = 0u64;
    for commit in 0..commits {
        total = total.wrapping_add(scan_commit_plain(commit));
    }
    total
}

#[trace]
#[inline(never)]
fn read_index_traced(seed: u64) -> u64 {
    work(seed, 1000)
}

#[trace]
#[inline(never)]
fn scan_objects_traced(seed: u64) -> u64 {
    work(seed ^ 0x5bf0_3635, 800)
}

#[trace]
#[inline(never)]
fn parse_pack_traced(seed: u64) -> u64 {
    scan_objects_traced(seed)
        .wrapping_add(scan_objects_traced(seed.rotate_left(7)))
        .wrapping_add(work(seed, 600))
}

#[trace]
#[inline(never)]
fn render_hunk_traced(seed: u64, hunk: u64) -> u64 {
    work(seed ^ hunk.wrapping_mul(31), 500)
}

#[trace]
#[inline(never)]
fn scan_commit_traced(seed: u64) -> u64 {
    let mut total = read_index_traced(seed);
    total = total.wrapping_add(parse_pack_traced(seed));
    for hunk in 0..4 {
        total = total.wrapping_add(render_hunk_traced(seed, hunk));
    }
    telemetry::context! {
        "commit" => seed,
        "hunks" => 4u64,
    };
    total
}

#[trace]
#[inline(never)]
fn realistic_traced(commits: u64) -> u64 {
    let mut total = 0u64;
    for commit in 0..commits {
        total = total.wrapping_add(scan_commit_traced(commit));
    }
    total
}

#[inline(never)]
async fn read_async_plain(seed: u64) -> u64 {
    tokio::task::yield_now().await;
    work(seed, 900)
}

#[inline(never)]
async fn parse_async_plain(seed: u64) -> u64 {
    tokio::task::yield_now().await;
    work(seed, 1100)
}

#[inline(never)]
async fn async_baseline(rounds: u64) -> u64 {
    let mut total = 0u64;
    for round in 0..rounds {
        total = total.wrapping_add(read_async_plain(round).await);
        total = total.wrapping_add(parse_async_plain(round).await);
    }
    total
}

#[trace]
#[inline(never)]
async fn read_async_traced(seed: u64) -> u64 {
    tokio::task::yield_now().await;
    work(seed, 900)
}

#[trace]
#[inline(never)]
async fn parse_async_traced(seed: u64) -> u64 {
    tokio::task::yield_now().await;
    work(seed, 1100)
}

#[trace]
#[inline(never)]
async fn async_traced(rounds: u64) -> u64 {
    let mut total = 0u64;
    for round in 0..rounds {
        total = total.wrapping_add(read_async_traced(round).await);
        total = total.wrapping_add(parse_async_traced(round).await);
    }
    total
}

fn primitive_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("primitives");

    group.bench_function("instant_now", |bencher| {
        bencher.iter(|| black_box(std::time::Instant::now()))
    });
    group.bench_function("instant_elapsed", |bencher| {
        let start = std::time::Instant::now();
        bencher.iter(|| black_box(start.elapsed()))
    });
    group.bench_function("quanta_instant_now", |bencher| {
        bencher.iter(|| black_box(quanta::Instant::now()))
    });
    group.bench_function("quanta_instant_elapsed", |bencher| {
        let start = quanta::Instant::now();
        bencher.iter(|| black_box(start.elapsed()))
    });

    let mutex = std::sync::Mutex::new(0u64);
    group.bench_function("mutex_lock_unlock", |bencher| {
        bencher.iter(|| {
            let mut guard = mutex.lock().unwrap();
            *guard = guard.wrapping_add(1);
            black_box(*guard);
        })
    });

    let arc = Arc::new(0u64);
    group.bench_function("arc_clone_drop", |bencher| {
        bencher.iter(|| {
            let clone = Arc::clone(&arc);
            black_box(&clone);
        })
    });

    group.finish();
}

fn cold_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("cold");

    group.bench_function("flat_32_calls_baseline", |bencher| {
        bencher.iter(|| black_box(flat_baseline(black_box(32))))
    });
    group.bench_function("flat_32_calls_instrumented", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| flat_traced(black_box(32)))))
    });
    group.bench_function("realistic_baseline", |bencher| {
        bencher.iter(|| black_box(realistic_baseline(black_box(8))))
    });
    group.bench_function("realistic_instrumented", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| realistic_traced(black_box(8)))))
    });

    group.finish();
}

fn init_runtime() {
    let config = Config::new(EnvironmentContext::detect("bench"))
        .enabled(true)
        .env_kill_switch(false)
        .sample_permille(1000)
        .buffer_capacity(512)
        .max_spans_per_trace(512)
        .max_depth(32);
    let sink = CountingSink {
        batches: AtomicU64::new(0),
    };
    telemetry::init(config, vec![Arc::new(sink)]).expect("fresh process");
    std::thread::sleep(std::time::Duration::from_millis(100));
}

fn micro_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("micro");

    group.bench_function("scope_only", |bencher| {
        bencher.iter(|| telemetry::sync_scope(|| {}))
    });
    group.bench_function("one_span", |bencher| {
        bencher.iter(|| {
            telemetry::sync_scope(|| {
                let _guard = telemetry::SpanGuard::enter("one");
            })
        })
    });
    group.bench_function("tiny_32_calls_baseline", |bencher| {
        bencher.iter(|| black_box(tiny_baseline(black_box(32))))
    });
    group.bench_function("tiny_32_calls", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| tiny_traced(black_box(32)))))
    });
    group.bench_function("tiny_32_calls_manual", |bencher| {
        bencher.iter(|| black_box(tiny_manual(black_box(32))))
    });
    group.bench_function("tiny_32_calls_span_only", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| tiny_span_only(black_box(32)))))
    });
    group.bench_function("flat_8_calls", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| flat_traced(black_box(8)))))
    });
    group.bench_function("flat_32_calls", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| flat_traced(black_box(32)))))
    });

    group.finish();
}

fn realistic_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("realistic");

    group.bench_function("baseline", |bencher| {
        bencher.iter(|| black_box(realistic_baseline(black_box(8))))
    });
    group.bench_function("instrumented", |bencher| {
        bencher.iter(|| black_box(telemetry::sync_scope(|| realistic_traced(black_box(8)))))
    });

    group.finish();
}

fn async_benchmarks(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime");
    let mut group = criterion.benchmark_group("async");

    group.bench_function("baseline", |bencher| {
        bencher.iter(|| black_box(runtime.block_on(async_baseline(black_box(8)))))
    });
    group.bench_function("instrumented", |bencher| {
        bencher.iter(|| black_box(runtime.block_on(telemetry::scope(async_traced(black_box(8))))))
    });

    group.finish();
}

fn flush_benchmarks(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("flush");

    group.bench_function("sixteen_traces", |bencher| {
        bencher.iter_batched(
            || {
                let _ = telemetry::flush();
                for _ in 0..16 {
                    telemetry::sync_scope(|| {
                        for _ in 0..8 {
                            let _guard = telemetry::SpanGuard::enter("flush.span");
                        }
                    });
                }
            },
            |()| black_box(telemetry::flush()),
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn benchmarks(criterion: &mut Criterion) {
    primitive_benchmarks(criterion);
    cold_benchmarks(criterion);
    init_runtime();
    micro_benchmarks(criterion);
    realistic_benchmarks(criterion);
    async_benchmarks(criterion);
    flush_benchmarks(criterion);
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
