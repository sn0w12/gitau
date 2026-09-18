set shell := ["pwsh", "-NoLogo", "-Command"]
set dotenv-load := false

ci:
    npm run format:check
    npm run lint
    npm run typecheck
    npm test
    cargo fmt --all --check
    cargo check --workspace --all-targets
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

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
    npm run tauri dev

dev-workspace:
    npm run tauri dev -- -- -- --session "{{ justfile_directory() }}/.dev-workspace/session.json"

build:
    npm run tauri build

settings:
    cargo run -p gitau --bin export_settings

# Bump the version, commit it, and tag it: just bump [patch|minor|major|x.y.z] [--dry-run] [--push] [--no-commit] [--no-tag]
bump *args="":
    node scripts/version.mjs {{ if args == "" { "patch" } else { args } }}
