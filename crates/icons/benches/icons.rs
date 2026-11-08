use std::hint::black_box;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use icons::{encode_data_url, FetchFuture, IconConfig, IconFetcher, IconService, OwnerIconSource};

const PNG_HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const REMOTE: &str = "https://github.com/octocat/hello-world.git";

struct FixedFetcher {
    bytes: Vec<u8>,
}

impl IconFetcher for FixedFetcher {
    fn fetch(&self, _url: String, _max_bytes: usize) -> FetchFuture {
        let bytes = self.bytes.clone();
        Box::pin(async move { Ok(bytes) })
    }
}

struct FakeSource;

impl OwnerIconSource for FakeSource {
    fn icon_url(&self, host: &str, owner: &str) -> Option<String> {
        (host == "github.com").then(|| format!("https://github.com/{owner}.png?size=128"))
    }
}

fn png_bytes(size: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; size.max(PNG_HEADER.len())];
    bytes[..PNG_HEADER.len()].copy_from_slice(&PNG_HEADER);
    bytes
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("icon-encode/data-url");
    for (label, size) in [
        ("128k", 128 * 1024),
        ("1m", 1024 * 1024),
        ("5m", 5 * 1024 * 1024),
    ] {
        let bytes = png_bytes(size);
        group.bench_function(label, |b| {
            b.iter(|| black_box(encode_data_url("image/png", &bytes)))
        });
    }
    group.finish();
}

fn bench_cached_resolve(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let bytes = png_bytes(128 * 1024);
    let fetcher = Arc::new(FixedFetcher {
        bytes: bytes.clone(),
    });
    let service = Arc::new(IconService::new(
        IconConfig {
            entry_ttl: Duration::from_secs(3600),
            ..IconConfig::default()
        },
        fetcher,
        Arc::new(FakeSource),
    ));
    service.initialize(dir.path().to_path_buf());
    runtime()
        .block_on(service.force_refresh(REMOTE))
        .unwrap()
        .unwrap();

    let mut group = c.benchmark_group("icon-service");
    // Each iteration reads the cached bytes, touches the entry's mtimes, and
    // re-encodes them, which is what a sidebar resolve pays.
    group.bench_function("resolve-cached-128k", |b| {
        b.iter(|| {
            let icon = runtime()
                .block_on(service.resolve(black_box(REMOTE)))
                .unwrap()
                .unwrap();
            assert!(icon.data_url.starts_with("data:image/png;base64,"));
            black_box(icon.data_url.len())
        })
    });
    group.finish();
}

fn bench_all(c: &mut Criterion) {
    bench_encode(c);
    bench_cached_resolve(c);
}

criterion_group!(benches, bench_all);
criterion_main!(benches);
