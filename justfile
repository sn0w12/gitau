set dotenv-load := false
set default-list

[windows]
set shell := ["pwsh", "-NoLogo", "-Command"]

# [-g|--gate <ids>] [-s|--skip <ids>] [--with-check] [--changed] [--base <ref>] [-j|--jobs <n>] [-v|--verbose] [--no-nextest] [--list] [-h|--help]
[group('ci')]
ci *args:
    node scripts/ci.mjs {{ args }}

[group('ci')]
check:
    npm run typecheck
    cargo check --workspace --all-targets
    just --fmt --check

[group('ci')]
fmt:
    cargo fmt --all
    npm run format
    just --fmt

[group('ci')]
lint:
    npm run lint
    cargo clippy --workspace --all-targets -- -D warnings

[group('ci')]
lint-fix:
    npm run lint:fix
    cargo clippy --fix --workspace --all-targets -- -D warnings

[group('dev')]
dev:
    npm run tauri dev -- --config crates/tauri/tauri.dev.conf.json

[group('dev')]
dev-workspace:
    npm run tauri dev -- --config crates/tauri/tauri.dev.conf.json -- -- --session "{{ justfile_directory() }}/.dev-workspace/session.json"

[group('build')]
build:
    npm run tauri build

[group('build')]
build-scan:
    npm run tauri build -- --config crates/tauri/tauri.scan.conf.json

[group('build')]
icons:
    npm run icons

# <filter> [--bench <name>] [--no-run] [--features <features>]
[group('perf')]
bench *args:
    cargo bench --workspace {{ args }}

# [dev|release|<log-file>] [out.json]
[group('perf')]
trace *args="release":
    node scripts/boot-trace.mjs {{ args }}

[group('settings')]
settings:
    cargo run -p gitau --bin export_settings --features devtools

# [patch|minor|major|preview|x.y.z] [--dry-run] [--push] [--no-commit] [--no-tag]
[group('release')]
bump *args='patch':
    node scripts/version.mjs {{ args }}
