set shell := ["pwsh", "-NoLogo", "-Command"]
set dotenv-load := false

ci *args="":
    node scripts/ci.mjs{{ if args == "" { "" } else { " " + args } }}

check:
    npm run typecheck
    cargo check --workspace --all-targets

fmt:
    cargo fmt --all
    npm run format

lint:
    npm run lint
    cargo clippy --workspace --all-targets -- -D warnings

lint-fix:
    npm run lint:fix
    cargo clippy --fix --workspace --all-targets -- -D warnings

dev:
    npm run tauri dev -- --config crates/tauri/tauri.dev.conf.json

dev-workspace:
    npm run tauri dev -- --config crates/tauri/tauri.dev.conf.json -- -- --session "{{ justfile_directory() }}/.dev-workspace/session.json"

build:
    npm run tauri build

bench *args="":
    cargo bench -p git-backend{{ if args == "" { "" } else { " " + args } }}

trace *args="release":
    node scripts/boot-trace.mjs {{ args }}

settings:
    cargo run -p gitau --bin export_settings --features devtools

# Bump the version, commit it, and tag it: just bump [patch|minor|major|preview|x.y.z] [--dry-run] [--push] [--no-commit] [--no-tag]
bump *args="":
    node scripts/version.mjs {{ if args == "" { "patch" } else { args } }}
