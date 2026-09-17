# Repository guidelines

## Project overview

gitau is a cross-platform Git desktop client: a Tauri 2 shell (Rust workspace) plus a React 19 frontend. Git work happens entirely in Rust (libgit2 + gitoxide); the frontend never shells out to the git CLI.

- `crates/git-backend/` is the pure-Rust git engine library: status, diff, history, remotes, mutations; dual engines (`git2` 0.21 as reference/mutation engine, `gix` 0.86 for fast reads); syntect syntax highlighting.
- `crates/tauri/` holds the Tauri commands and app setup, bridging webview ↔ backend over IPC (~52 registered commands).

## Architecture and data flow

```
React UI (src/)
  → BackendClient (src/lib/backend/transport/client.ts)   ← only place with command names
    → invokeCommand (src/lib/backend/transport/invoke.ts) → Tauri IPC
      → #[tauri::command] fns (crates/tauri/src/commands/) → Backend facade (git-backend/src/application/backend.rs)
        → engines/git2 | engines/gix → repository on disk
```

- Frontend state: TanStack Store singletons in `src/stores/` (not zustand/redux), each exporting a `store` const, plain action functions, and selectors. Data fetching: TanStack Query v5. Routing: TanStack Router, one memory-history router per app tab, each built from a fresh route tree (`src/routes/route-tree.tsx:createTabRouteTree`) because routers mutate their route nodes. Never share or cache route trees. Hidden tabs stay mounted via React `<Activity>` boundaries (`src/components/active-tab-host.tsx`).
- Result discipline: every `BackendClient` method returns `Result<T> = {ok:true,value} | {ok:false,error}` and never rejects. Unwrap with `expectOk()` only at react-query boundaries (queryFn / mutation callbacks).
- Backend errors: `GitError` enum (`crates/git-backend/src/error/mod.rs`, thiserror, no anyhow) serializes to `{code, message, retryable, detail}` with stable camelCase codes; commands return `CommandResult<T> = Result<T, SerializedError>`. Long ops take a `CancellationToken`; cancel explicitly with `git_cancel_operation`.
- Mutations: the shared `useRepoWrite` hook sends `expectedGeneration` from the snapshot query (optimistic concurrency vs backend generation counter); invalidation goes through centralized scopes in `src/lib/backend/mutations/invalidation.ts`. No optimistic writes to status/history; the backend is source of truth.
- Push invalidation: the backend emits the `repository-invalidated` Tauri event, listened once in `src/components/providers/app-services-provider.tsx`, which marks diff sessions stale and invalidates all repo data.
- Streaming diff: `git_open_diff` returns an operation id and pumps events over a Tauri `Channel<DiffEvent>`; session lifecycle in `src/lib/backend/streams/diff-session*.ts`.

## Key directories

| Path                      | Purpose                                                                                                                                                                                                           |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/stores/`             | TanStack Store singletons: `app-store` (tabs), `repository-store` (repos keyed by durable path), `settings-store`, `operation-store` (op lifecycle), `theme-store`, `tab-runtime` (per-tab non-persisted runtime) |
| `src/lib/`                | Framework-free core: `backend/` (whole Rust bridge), `routing/`, `bootstrap/`, `diff/`, `settings/settings.generated.ts` (generated)                                                                              |
| `src/hooks/`              | One concern per file, kebab-case `use-*.ts` (e.g. `hooks/repositories/use-repository-queries.ts`)                                                                                                                 |
| `src/contexts/`           | Minimal: services, per-tab id, promise-queued confirm dialog, tab title                                                                                                                                           |
| `src/components/`         | Feature folders (`repo/`, `diff/`, `settings/`, `titlebar/`) + generic `ui/` primitives (shadcn/coss over Base UI)                                                                                                |
| `src/routes/`             | Page components + `route-tree.tsx`                                                                                                                                                                                |
| `crates/git-backend/src/` | `api/` (DTOs), `application/backend.rs` (facade), `domain/`, `engines/{git2,gix}/`, `error/`, `runtime/` (scheduler/cache/watcher/cancellation), `streaming/`                                                     |
| `crates/tauri/src/`       | `lib.rs` (builder + command registration), `state.rs`, `commands/*`, `session.rs`, `settings/`                                                                                                                    |
| `tests/`                  | Frontend Vitest suite mirroring `src/`: `lib/`, `stores/`, `components/`, `hooks/`                                                                                                                                |

## Development commands

```bash
npm install
npm run tauri dev        # desktop app with hot reload
npm run dev              # frontend only, port 3000 (strict)
npm run build            # vite build (frontend)
npm run tauri build      # installer bundle

# Gates, all run in CI (.github/workflows/ci.yml):
npm run format:check     # oxfmt --check
npm run lint             # oxlint
npm run typecheck        # tsc --noEmit
npm test                 # vitest run
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Regenerate the settings TS contract after changing the Rust settings schema:

```bash
cargo run -p gitau --bin export_settings   # rewrites src/lib/settings/settings.generated.ts
```

Benchmarks: `cargo bench -p git-backend` (Criterion, `crates/git-backend/benches/`).

## Code conventions and common patterns

### Formatting

oxfmt-enforced, match manually: double quotes, semicolons, 4-space indent, printWidth 80, ES5 trailing commas, sorted imports (packages, then `@/…`, relative last). Rust: default rustfmt, clippy denies warnings.

### Comments

Default is no comment. Write one only when the code cannot say what it says. A comment earns its place by explaining a non-obvious why, such as an invariant, a workaround, or a rationale for something that looks wrong:

- Good: library quirks (`@dnd-kit` snapshot shapes drop prototype getters), measurement guards (hidden tabs report 0x0), crash-safety ordering (flush before rename so a crash can't truncate), parity notes (matching libgit2's xdiff output), protocol/wire contracts.
- Bad, never add these:
    - Restating the name or the next line: `/// Tunables for the icon service`, `// Re-render when runtimes are created/disposed`.
    - Usage/condition docs on props, fields, constants: `/** Present when X */`, `/** Used by the fetch button */`, `/** Whether this panel is visible */`.
    - Styling/intent narration on class constants (`TRIGGER_EXTRAS`, `KIND_ROW_CLASS`); the Tailwind string says what it does.
    - Section banners and dividers: `// NAVIGATION`, `// ---- internals ----`.
    - Changelog-style or speculative notes ("will be used later" prose) without a concrete contract.

`///` rustdoc follows the same rule: document public API only where it adds semantics beyond the signature, such as invariants, defaults, and failure behavior; omit it when it would just restate the item name. Vendored directories (`src/components/ui/`, `src/components/evilcharts/`) keep their upstream comments untouched.

When editing code whose comment you invalidate (renamed threshold, changed ladder, moved file path), fix the comment in the same change; stale why-comments are worse than none.

### TypeScript

strict mode plus `noUnusedLocals`, `noUnusedParameters`, `verbatimModuleSyntax`; use `import type` for type-only imports. Alias `@/*` → `src/*`.

### Frontend patterns

- Named-export function components everywhere (no default exports). Hooks live above any early return (fixed hook order).
- Never call `invoke` or add command names outside `src/lib/backend/transport/client.ts`; hand-maintain the protocol twin `src/lib/backend/protocol.ts`.
- Queries via `queryOptions()` factories (`src/lib/backend/queries/repository-queries.ts`) + structured keys (`query-keys.ts`); `useQuery` hooks guard with `hasValidRepoId(repoId)` before calling the backend.
- Store ownership: durable tab data → `appStore`; repo set → `repositoryStore` (keyed by path); ephemeral per-tab UI → `tab-runtime.ts`; op tracking → `operationStore`. Setters return unchanged state when nothing changed.
- Errors surfaced via `toastError(title, error)` (`src/lib/toast-error.ts`); destructive actions go through `useConfirm()`.
- Never use html titles, never use custom font sizes.

### Critical invariants

- `repoId` is ephemeral/process-local; the repo path is durable. Never persist `repoId`; route loaders `throw redirect(...)` to rewrite stale persisted ids.
- `getSetting`/`setSetting` throw before settings init; bootstrap order in `src/main.tsx` is load-bearing (settings → theme engine → render).
- Generated files are never hand-edited: `src/lib/settings/settings.generated.ts` (a freshness test fails if stale). Don't recreate the scaffold-era `routeTree.gen.ts`; routing here is code-based.

### Rust patterns

- Commands are thin: `#[tauri::command] async fn git_xxx(state: SharedState<'_>, …) -> CommandResult<T>` delegating to `state.backend.<method>(…).map_err(to_serialized)`; register in the flat `generate_handler!` list in `crates/tauri/src/lib.rs`. Git ops prefixed `git_`; settings/session/icon commands unprefixed.
- Serde enums sent over IPC (tagged events like `DiffEvent`/`CloneEvent`, error wire shapes) need both `rename_all = "camelCase"` AND `rename_all_fields = "camelCase"`: the first only renames variant names, so fields would hit the frontend as snake_case and read back as `undefined` with no error.
- Blocking git work goes through the scheduler (`runtime/scheduler.rs`, semaphore-bounded → `spawn_blocking`). In-app tasks use `tauri::async_runtime::{spawn, spawn_blocking}`, never raw `tokio::spawn` in setup.
- Engine parity is tested: gix results must equal the git2 reference (`crates/git-backend/tests/{status_parity,diff_parity}.rs`). The only `std::process::Command` usage is running repo hooks after commit/merge.

## Anti-slop

Every word written in this repo follows this: comments, markdown docs, commit messages, PR descriptions.

- Em dashes are out, including en dashes and hyphens doing a dash's job. End the sentence or use a comma.
- Straight quotes only, never curly.
- Plain verbs win: use, change, help. Not utilize, leverage, facilitate.
- Retired vocabulary: crucial, seamless, robust, comprehensive, showcase, foster, delve, pivotal, testament, landscape (abstract sense). Reaching for one means the point isn't made yet.
- Cut filler: "in order to" is "to", "due to the fact that" is "because", "it is important to note that" is deleted outright.
- Keep sentences active and name the actor. An adverb propping up a weak verb gets replaced by a number ("significantly faster" becomes "2.4x faster").
- Describe mechanisms, not vibes. "Preserves scroll position across tab switches" beats "feels buttery smooth".
- No bold-label spam, no title-cased headings in markdown, no decorative emoji, no forced groups of three, no "not just X but Y".

## Important files

- `src/main.tsx`: async bootstrap (settings → session restore → theme → render); failure renders `BootstrapError`.
- `src/lib/backend/transport/client.ts`: typed backend client; single choke point for command names.
- `src/lib/bootstrap/app-runtime.ts`: process-wide `{backend, queryClient}` singleton (`resetAppRuntime()` for tests).
- `src/routes/route-tree.tsx`: per-tab route tree factory; loaders handle title/badge, repo binding, stale-id redirects.
- `src/components/providers/app-services-provider.tsx`: context provider and sole listener of `repository-invalidated`. Must export only the component or HMR Fast Refresh breaks.
- `crates/tauri/src/lib.rs`: plugin wiring and all command registrations. `crates/tauri/src/state.rs` defines `AppState { backend, settings, session, icons }`.
- `crates/git-backend/src/application/backend.rs`: the `Backend` facade every command delegates to.
- `crates/tauri/tauri.conf.json`: devUrl port 3000, `beforeBuildCommand: npm run build`.
- `components.json`: coss registry config; add primitives via `npx shadcn@latest add @coss/button` into `src/components/ui/`.

## Runtime and tooling preferences

- npm is the package manager (`package-lock.json`, CI uses `npm ci`). Ignore `.cta.json`'s stale `pnpm`/file-router fields, scaffold leftovers.
- Node LTS, Rust 1.85+ (workspace `rust-version = "1.85"`; git-backend uses edition 2024).
- Lint/format: oxlint + oxfmt, not ESLint/Prettier (VS Code defaults to the oxc extension, Prettier disabled). oxlint `correctness` rules are errors; React purity/immutability/exhaustive-deps warnings exist, so don't add code that trips them.
- Tailwind CSS v4 CSS-first (no tailwind.config): theme tokens in `src/styles.css`, dark mode is a `.dark` class on `<html>` via theme-store. Icons: lucide-react.
- Release builds run through the GitHub Actions tag workflow (`v*.*.*`) using tauri-action; bundles nsis/msi/dmg/appimage/deb.

## Testing and QA

- Frontend: Vitest 4, tests only in top-level `tests/**/*.test.{ts,tsx}` (never colocated in `src/`); tree mirrors `src/`. Config lives in the `test:` key of `vite.config.ts`, no separate vitest config.
- DOM tests opt in per file with a `// @vitest-environment jsdom` docblock (default env is node).
- Mocks: do not mock `@tauri-apps/api/core` directly. Build fake `BackendClient` objects returning `Result<T>` unions and inject by mocking the services context:
    ```ts
    vi.mock("@/contexts/services-context", () => ({
        useAppServices: () => ({ backend: fakeBackend }),
    }));
    ```
    Settings seeded via exported `seedSettingsForTests(...)`; stores reset via `appStore.setState(...)`; `resetAppRuntime()` between runtime tests.
- Rust: integration tests in `crates/git-backend/tests/` (shared `TestRepo` helper builds real temp repos via git2 in `tests/common/mod.rs`); unit tests inline `#[cfg(test)]` throughout both crates; freshness test guards `settings.generated.ts`.
- Coverage: no coverage tooling configured; PRs must pass the full gate list under Development commands.
