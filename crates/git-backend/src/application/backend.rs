use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::api::changes::{DiscardRequest, StageRequest, StatusOptions};
use crate::api::github::{
    AccountProfile, DeviceFlowStart, GithubIssueComment, GithubIssueDetail, GithubIssueEvent,
    GithubIssueListItem, GithubOrg, GithubRepoPermissions, NotificationPage,
    PublishRepositoryRequest, PublishResult, SearchIssuePage, UpdateIssueBody,
};
use crate::api::graph::GraphQuery;
use crate::api::highlight::HighlightedSnippet;
use crate::api::history::{
    BlameQuery, CommitDetailQuery, FileAtRevisionQuery, HistoryChartQuery, HistoryPageQuery,
};
use crate::api::hooks::{HookContent, HookInfo, HookRunResult};
use crate::api::lfs::LfsStatus;
use crate::api::mutations::{
    AmendRequest, BranchCreateRequest, CheckoutRequest, CommitExecution, CommitRequest,
    ConflictFile, MergeAbortRequest, MergeContinueRequest, MergeRequest, OperationState,
    ResetRequest, ResolveConflictRequest, RevertRequest, StashPopRequest, StashPushRequest,
    TagCreateRequest,
};
use crate::api::queries::DiffRequest;
use crate::api::remote_info::RemoteRepoInfo;
use crate::api::remotes::{
    CloneEvent, CloneProgress, CloneRequest, FetchRequest, PullRequest, PushOutcome, PushRequest,
    RemoteAddRequest,
};
use crate::api::repository::{CreateRepositoryRequest, GitignoreTemplateInfo, LicenseTemplateInfo};
use crate::api::submodules::{SubmoduleAddRequest, SubmoduleInfo, SubmoduleUpdateRequest};
use crate::api::worktrees::{
    WorktreeCreateRequest, WorktreeInfo, WorktreeLockRequest, WorktreeRemoveRequest,
};
use crate::domain::history::{
    BlameResult, CommitWithDetail, FileContent, HistoryChart, HistoryPage, RepoListing,
};
use crate::domain::templates;
use crate::domain::{
    BranchInfo, ChangeKind, CommitSummary, Generation, ObjectId, OperationId, RelativePath, RepoId,
    RepoSnapshot, SnapshotId, StatusReport, TagInfo,
};
use crate::engines::git2::{hooks as git2_hooks, mutations};
use crate::engines::gix::objects as gix_objects;
use crate::error::{GitError, Result};
use crate::github::{CreateRepoBody, GitHubAuth};
use crate::runtime::cancellation::CancellationToken;
use crate::runtime::clone_ops::{CloneOperation, CloneOperationRegistry};
use crate::runtime::registry::{CachedValue, Registry};
use crate::runtime::scheduler::Scheduler;
use crate::streaming::graph::{GraphOperation, GraphOperationRegistry, GraphRangeResult};
use crate::streaming::graph_pipeline::{self, GraphJob};
use crate::streaming::model::{DiffEvent, GraphEvent};
use crate::streaming::pipeline::{self, DiffJob};
use crate::streaming::store::{DiffImage, DiffOperation, DiffOperationRegistry, RangeResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackendConfig {
    /// Maximum concurrent blocking git jobs.
    pub max_blocking_operations: usize,
    /// Watch worktrees for external modifications.
    pub watch_worktree: bool,
    pub watcher_debounce_ms: u64,
    /// Completed diff operations retained for range reads.
    pub keep_completed_operations: usize,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            max_blocking_operations: 16,
            watch_worktree: true,
            watcher_debounce_ms: 150,
            keep_completed_operations: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedRepository {
    pub id: RepoId,
    pub snapshot: RepoSnapshot,
}

/// A freshly created repository plus the canonical path callers should use
/// for opening it later (input parent/name may differ in separators).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepositoryResult {
    #[serde(flatten)]
    pub repo: OpenedRepository,
    pub path: String,
}

/// The outcome of removing a repository: the closed backend session id
/// (absent when the repo had no open session) and the canonical repo root
/// the removal and optional trash applied to.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveRepositoryResult {
    pub removed_repo_id: Option<u64>,
    pub repo_root: String,
}

type LanguageStatsCache = Arc<
    Mutex<
        Vec<(
            PathBuf,
            Option<Vec<crate::engines::gix::language_stats::LanguageEntry>>,
        )>,
    >,
>;

pub struct Backend {
    scheduler: Scheduler,
    registry: Registry,
    operations: DiffOperationRegistry,
    graph_operations: GraphOperationRegistry,
    clone_operations: CloneOperationRegistry,
    next_operation_id: AtomicU64,
    config: BackendConfig,
    github: Arc<GitHubAuth>,
    remote_info_cache: Arc<Mutex<RemoteInfoCacheState>>,
    language_stats_cache: LanguageStatsCache,
}

/// Language stats are recomputed only past this many distinct repos.
const LANGUAGE_STATS_CACHE_CAP: usize = 256;

#[derive(Default)]
struct RemoteInfoCacheState {
    /// Set once at startup; absent means the cache stays in-memory only.
    dir: Option<PathBuf>,
    /// Cache keys with a refresh currently in flight (SWR dedup).
    refreshing: HashSet<String>,
}

impl Default for Backend {
    fn default() -> Self {
        Self::new(BackendConfig::default())
    }
}

impl Backend {
    pub fn new(config: BackendConfig) -> Self {
        Self::with_github(
            config,
            Arc::new(GitHubAuth::new(
                Arc::new(crate::github::HttpGithubApi::default()),
                Box::new(crate::github::KeyringTokenStore),
            )),
        )
    }

    /// Test seam: swap in a fake GitHub auth/api stack.
    pub fn with_github(config: BackendConfig, github: Arc<GitHubAuth>) -> Self {
        let hub = Arc::new(crate::runtime::invalidation::InvalidationHub::new());
        Self {
            scheduler: Scheduler::new(config.max_blocking_operations),
            registry: Registry::new(hub),
            operations: DiffOperationRegistry::default(),
            graph_operations: GraphOperationRegistry::default(),
            clone_operations: CloneOperationRegistry::default(),
            next_operation_id: AtomicU64::new(1),
            config,
            github,
            remote_info_cache: Arc::default(),
            language_stats_cache: Arc::default(),
        }
    }

    /// Points the remote-info disk cache at `dir`. Missing directories are
    /// created lazily on first write; without this the cache stays
    /// in-memory only and every lookup hits the provider API.
    pub fn initialize_remote_info_cache(&self, dir: PathBuf) {
        self.remote_info_cache.lock().unwrap().dir = Some(dir);
    }

    /// Deletes cached remote-info payloads untouched for longer than
    /// `max_age`. Last use is the file mtime (touched on every serve),
    /// falling back to the payload's fetch time. Returns the number of
    /// files removed. Best-effort, never fails startup cleanup.
    pub fn prune_remote_info_cache(&self, max_age: Duration) -> usize {
        let dir = self.remote_info_cache.lock().unwrap().dir.clone();
        let Some(dir) = dir else {
            return 0;
        };
        prune_dir_older_than(&dir, max_age)
    }

    /// Points the process-wide global excludes override at `path` (`None`
    /// clears it). Every gix handle created afterwards applies it as an
    /// in-memory `core.excludesFile`.
    pub fn set_global_excludes_file(&self, path: Option<PathBuf>) {
        crate::global_ignore::set_global_excludes_file(path);
    }

    /// Rebuilds the merged global excludes file from the setting content
    /// and the git system excludes file, applies it process-wide, and bumps
    /// every open repository so the new rules take effect immediately.
    pub fn apply_global_ignore_setting(&self, app_content: &str, config_dir: &Path) -> Result<()> {
        let merged = crate::global_ignore::ensure_merged_excludes_file(
            &config_dir.join("gitignore"),
            app_content,
        )?;
        self.set_global_excludes_file(merged);
        self.registry.bump_all_generations("globalIgnore");
        Ok(())
    }

    /// Applies the syntax highlighting theme pair and bumps every open
    /// repository so already-materialized diffs restream with the new
    /// colors. Unknown keys fall back to the built-in defaults.
    pub fn apply_syntax_theme_setting(&self, light: &str, dark: &str) {
        crate::engines::gix::highlight::set_theme_pair(light, dark);
        self.registry.bump_all_generations("syntaxTheme");
    }

    pub fn github(&self) -> &Arc<GitHubAuth> {
        &self.github
    }

    /// Stream of repository invalidations (watcher bumps and successful mutations).
    pub fn subscribe_invalidations(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::runtime::invalidation::RepositoryInvalidation>
    {
        self.registry.hub().subscribe()
    }

    pub fn config(&self) -> &BackendConfig {
        &self.config
    }

    pub fn open_repository_count(&self) -> usize {
        self.registry.len()
    }

    pub async fn open_repository(&self, path: &Path) -> Result<OpenedRepository> {
        if !path.exists() {
            return Err(GitError::RepositoryNotFound {
                path: path.to_owned(),
            });
        }
        let probe = crate::engines::gix::GixSession::discover(path)?;
        let watch_paths = self.watch_paths(&probe);
        let (id, entry, created) = self.registry.register(path, watch_paths)?;
        let snapshot = build_snapshot(&entry)?;

        // Warm the gix object database in the background: the first diff /
        // history interaction otherwise pays one-time pack-index and tree
        // loading costs, which reads as a multi-second hang.
        let warm_session = entry.gix.clone();
        // Called within an async context, so a runtime exists.
        std::mem::drop(tokio::spawn(async move {
            let _ = warm_session.handle().head_id();
        }));

        // First registration only: the walk/summary caches are immutable and
        // survive generation bumps, so re-opens never need re-warming.
        if created {
            self.spawn_history_warmup(id);
        }

        // Status is generation-keyed, so every open warms it fresh. The
        // scan runs off the interaction path; the first changes panel
        // request then hits the cache instead of paying the worktree scan.
        self.spawn_status_warmup(id);

        Ok(OpenedRepository { id, snapshot })
    }

    /// Computes the default status report off the interaction path so the
    /// first changes panel paint finds a finished report. A generation bump
    /// during the scan drops the result: the next request recomputes.
    fn spawn_status_warmup(&self, id: RepoId) {
        let Ok(entry) = self.registry.get(id) else {
            return;
        };
        let generation = entry.generation();
        let cache_key = (generation, StatusOptions::default());
        if entry.cached("status", &cache_key).is_some() {
            return;
        }
        let scheduler = self.scheduler.clone();
        let gix_session = entry.gix.clone();
        tokio::spawn(async move {
            let token = CancellationToken::new();
            let Ok(mut report) = scheduler
                .run(&token, move || {
                    crate::engines::gix::status::status(&gix_session, &StatusOptions::default())
                })
                .await
            else {
                return;
            };
            if entry.generation() != generation {
                return;
            }
            report.snapshot_id = entry.next_snapshot_id();
            report.generation = generation;
            entry.store_cached("status", &cache_key, CachedValue::Status(Arc::new(report)));
        });
    }

    /// Builds walk + default-page summaries off the interaction path so a
    /// freshly opened repo never pays the cold revwalk inside a request.
    fn spawn_history_warmup(&self, id: RepoId) {
        let Ok(entry) = self.registry.get(id) else {
            return;
        };
        let scheduler = self.scheduler.clone();
        let path = entry.canonical_path.clone();
        let history_cache = entry.history_cache();
        let lock_entry = entry.clone();
        let generation = entry.generation();
        tokio::spawn(async move {
            let token = CancellationToken::new();
            let _ = scheduler
                .run(&token, move || {
                    let _computation = lock_entry.history_lock().lock().unwrap();
                    let query = HistoryPageQuery::default();
                    let mut guard = history_cache.lock().unwrap();
                    let session = crate::engines::git2::Git2Session::new(path);
                    crate::engines::git2::history::history_page(
                        &session,
                        &query,
                        SnapshotId(0),
                        generation,
                        &mut guard,
                    )
                })
                .await;
        });
    }

    fn watch_paths(&self, session: &crate::engines::gix::GixSession) -> Vec<PathBuf> {
        if !self.config.watch_worktree {
            return Vec::new();
        }
        if let Some(workdir) = session.workdir() {
            vec![workdir.to_path_buf()]
        } else {
            vec![session.git_dir().to_path_buf()]
        }
    }

    pub async fn close_repository(&self, id: RepoId) -> Result<bool> {
        Ok(self.registry.remove(id))
    }

    /// Removes a repository from the app: closes its backend session and,
    /// when `move_to_trash` is set, moves the working copy to the OS trash.
    /// The trash targets the canonical repo root even when the caller added
    /// the repo through a subfolder, so the whole copy leaves the disk. A
    /// working copy that is already gone counts as removed.
    pub async fn remove_repository(
        &self,
        path: &Path,
        move_to_trash: bool,
    ) -> Result<RemoveRepositoryResult> {
        let canonical_input = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        let repo_root = crate::engines::gix::GixSession::discover(&canonical_input)
            .ok()
            .and_then(|session| session.workdir().map(Path::to_path_buf))
            .unwrap_or_else(|| canonical_input.clone());
        let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);

        let removed_repo_id = self.registry.remove_by_path(&repo_root).map(|id| id.0);
        // Clear any libgit2 handles cached on the current backend thread before
        // asking Windows to move the working tree.
        crate::engines::git2::local::clear_all_repository_handles(&repo_root);

        if move_to_trash && repo_root.exists() {
            let root_for_trash = repo_root.clone();
            self.scheduler
                .run(&CancellationToken::new(), move || {
                    trash::delete(&root_for_trash).map_err(|error| GitError::Trash {
                        details: error.to_string(),
                    })
                })
                .await?;
        }

        Ok(RemoveRepositoryResult {
            removed_repo_id,
            repo_root: repo_root.to_string_lossy().into_owned(),
        })
    }

    pub async fn repository_snapshot(&self, id: RepoId) -> Result<RepoSnapshot> {
        let entry = self.registry.get(id)?;
        build_snapshot(&entry)
    }

    pub fn clear_repository_caches(&self, id: RepoId) -> Result<()> {
        let entry = self.registry.get(id)?;
        entry.clear_cache();
        Ok(())
    }

    /// Drops immutable history caches too; benchmark and test escape hatch.
    pub fn clear_repository_history_caches(&self, id: RepoId) -> Result<()> {
        let entry = self.registry.get(id)?;
        entry.history_cache().lock().unwrap().clear();
        Ok(())
    }

    pub async fn status(
        &self,
        id: RepoId,
        options: StatusOptions,
        token: CancellationToken,
    ) -> Result<Arc<StatusReport>> {
        let entry = self.registry.get(id)?;
        let cache_key = (entry.generation(), options);
        if let Some(CachedValue::Status(cached)) = entry.cached("status", &cache_key) {
            return Ok(cached);
        }
        let snapshot_id = entry.next_snapshot_id();
        let generation = entry.generation();

        // gix computes status with parallel workdir traversal and native
        // metadata, far faster than libgit2's per-file lstat loop on Windows.
        // Output parity with the libgit2 engine is enforced by tests.
        let gix_session = entry.gix.clone();
        let report = self
            .scheduler
            .run(&token, move || {
                crate::engines::gix::status::status(&gix_session, &options)
            })
            .await?;

        let mut report = report;
        report.snapshot_id = snapshot_id;
        report.generation = generation;
        let arc = Arc::new(report);
        entry.store_cached("status", &cache_key, CachedValue::Status(arc.clone()));
        Ok(arc)
    }

    pub async fn open_diff(
        &self,
        id: RepoId,
        request: DiffRequest,
        token: CancellationToken,
    ) -> Result<(OperationId, mpsc::Receiver<DiffEvent>)> {
        let entry = self.registry.get(id)?;
        token.check()?;
        let operation_id = OperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let operation = Arc::new(DiffOperation::new(id, operation_id, entry.generation()));
        self.operations.insert(operation.clone());

        let (tx, rx) = mpsc::channel::<DiffEvent>(256);
        let job = DiffJob::new(
            entry.canonical_path.clone(),
            request,
            operation_id,
            entry.next_snapshot_id(),
            entry.generation(),
            entry.gix.clone(),
        );

        let op_for_job = operation.clone();
        let scheduler = self.scheduler.clone();
        tokio::spawn(async move {
            let _ = scheduler
                .run(&token, move || {
                    pipeline::run(job, op_for_job, tx);
                    Ok(())
                })
                .await;
        });

        self.operations
            .prune_completed(self.config.keep_completed_operations);
        Ok((operation_id, rx))
    }

    pub fn read_diff_image(
        &self,
        operation_id: OperationId,
        section_id: u32,
    ) -> Result<Option<DiffImage>> {
        self.operations.get(operation_id)?.read_image(section_id)
    }

    pub fn read_diff_range(
        &self,
        operation_id: OperationId,
        start: u64,
        max_rows: u32,
    ) -> Result<RangeResult> {
        let operation = self.operations.get(operation_id)?;
        operation.read_range(start, max_rows)
    }

    pub async fn open_graph(
        &self,
        id: RepoId,
        request: GraphQuery,
        token: CancellationToken,
    ) -> Result<(OperationId, mpsc::Receiver<GraphEvent>)> {
        let entry = self.registry.get(id)?;
        token.check()?;
        let operation_id = OperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let operation = Arc::new(GraphOperation::new(id, operation_id, entry.generation()));
        self.graph_operations.insert(operation.clone());

        let (tx, rx) = mpsc::channel::<GraphEvent>(256);
        let job = GraphJob::new(
            entry.canonical_path.clone(),
            request,
            operation_id,
            entry.next_snapshot_id(),
            entry.generation(),
            entry.history_cache(),
        );

        let op_for_job = operation.clone();
        let scheduler = self.scheduler.clone();
        tokio::spawn(async move {
            let _ = scheduler
                .run(&token, move || {
                    graph_pipeline::run(job, op_for_job, tx);
                    Ok(())
                })
                .await;
        });

        self.graph_operations
            .prune_completed(self.config.keep_completed_operations);
        Ok((operation_id, rx))
    }

    pub fn read_graph_range(
        &self,
        operation_id: OperationId,
        start: u64,
        max_rows: u32,
    ) -> Result<GraphRangeResult> {
        let operation = self.graph_operations.get(operation_id)?;
        operation.read_range(start, max_rows)
    }

    pub fn cancel_operation(&self, operation_id: OperationId) -> Result<bool> {
        if let Ok(operation) = self.operations.get(operation_id) {
            return Ok(operation.cancel());
        }
        if let Ok(operation) = self.clone_operations.get(operation_id) {
            return Ok(operation.cancel());
        }
        Ok(self.graph_operations.get(operation_id)?.cancel())
    }

    pub fn diff_operation_sections(
        &self,
        operation_id: OperationId,
    ) -> Result<Vec<(crate::streaming::model::SectionMeta, u64, u64)>> {
        Ok(self.operations.get(operation_id)?.snapshot_meta())
    }

    pub async fn history_page(
        &self,
        id: RepoId,
        query: HistoryPageQuery,
        token: CancellationToken,
    ) -> Result<Arc<HistoryPage>> {
        let entry = self.registry.get(id)?;
        let cache_key = (entry.generation(), query.clone());
        if let Some(CachedValue::History(cached)) = entry.cached("history", &cache_key) {
            return Ok(cached);
        }
        let path = entry.canonical_path.clone();
        let snapshot_id = entry.next_snapshot_id();
        let generation = entry.generation();

        let history_cache = entry.history_cache();
        let lock_entry = entry.clone();
        let page = self
            .scheduler
            .run(&token, move || {
                // git2 engine: libgit2's diff stats give per-commit
                // files/additions/deletions cheaply for the whole page.
                // Walk/summary/tag caches hold immutable data and survive
                // generation bumps. Computations serialize per repo so
                // concurrent callers share the caches instead of each
                // redoing the diff stats.
                let _computation = lock_entry.history_lock().lock().unwrap();
                let mut guard = history_cache.lock().unwrap();
                let session = crate::engines::git2::Git2Session::new(path);
                crate::engines::git2::history::history_page(
                    &session,
                    &query,
                    snapshot_id,
                    generation,
                    &mut guard,
                )
            })
            .await?;
        let arc = Arc::new(page);
        entry.store_cached("history", &cache_key, CachedValue::History(arc.clone()));
        Ok(arc)
    }

    pub async fn history_chart(
        &self,
        id: RepoId,
        query: HistoryChartQuery,
        token: CancellationToken,
    ) -> Result<Arc<HistoryChart>> {
        let entry = self.registry.get(id)?;
        let cache_key = (entry.generation(), query.clone());
        if let Some(CachedValue::HistoryChart(cached)) = entry.cached("history-chart", &cache_key) {
            return Ok(cached);
        }
        let path = entry.canonical_path.clone();
        let snapshot_id = entry.next_snapshot_id();
        let generation = entry.generation();

        let history_cache = entry.history_cache();
        let chart = self
            .scheduler
            .run(&token, move || {
                // Same engine and cache as history pages: the walk is shared,
                // and per-commit stat points are immutable once computed.
                let mut guard = history_cache.lock().unwrap();
                let session = crate::engines::git2::Git2Session::new(path);
                crate::engines::git2::chart::history_chart(
                    &session,
                    &query,
                    snapshot_id,
                    generation,
                    &mut guard,
                )
            })
            .await?;
        let arc = Arc::new(chart);
        entry.store_cached(
            "history-chart",
            &cache_key,
            CachedValue::HistoryChart(arc.clone()),
        );
        Ok(arc)
    }

    pub async fn commit_detail(
        &self,
        id: RepoId,
        query: CommitDetailQuery,
        token: CancellationToken,
    ) -> Result<Arc<CommitWithDetail>> {
        let entry = self.registry.get(id)?;
        let cache_key = (entry.generation(), query.clone());
        if let Some(CachedValue::CommitDetail(cached)) = entry.cached("commit-detail", &cache_key) {
            return Ok(cached);
        }
        let path = entry.canonical_path.clone();
        let detail = self
            .scheduler
            .run(&token, move || commit_detail_blocking(path, query))
            .await?;
        let arc = Arc::new(CommitWithDetail { detail });
        entry.store_cached(
            "commit-detail",
            &cache_key,
            CachedValue::CommitDetail(arc.clone()),
        );
        Ok(arc)
    }

    pub async fn file_at_revision(
        &self,
        id: RepoId,
        query: FileAtRevisionQuery,
        token: CancellationToken,
    ) -> Result<FileContent> {
        let entry = self.registry.get(id)?;
        let path_str = RelativePath::parse(&query.path)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&token, move || {
                let session = crate::engines::gix::GixSession::discover(&path)?;
                let repo = session.handle();
                let resolved = repo
                    .rev_parse_single(query.revision.as_str())
                    .map_err(|_| GitError::InvalidRevision {
                        spec: query.revision.as_str().to_owned(),
                    })?
                    .detach();
                gix_objects::blob_at(&session, ObjectId::from(resolved), &path_str)
            })
            .await
    }

    /// Probes the worktree for an icon file (favicon/logo/icon names,
    /// gitignored paths excluded) and returns it as a data URL, or `None`
    /// when the worktree holds nothing usable.
    pub async fn worktree_icon(&self, id: RepoId) -> Result<Option<crate::icons::CachedIcon>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        let key = format!("local:{}", path.display());
        let icon = self
            .scheduler
            .run(&CancellationToken::new(), move || {
                crate::engines::gix::worktree_icon::find_worktree_icon(&path)
            })
            .await?;
        Ok(icon.map(|icon| crate::icons::CachedIcon {
            key,
            data_url: crate::icons::encode_data_url(icon.content_type, &icon.bytes),
            content_type: icon.content_type.to_owned(),
            fetched_at_ms: icon.modified_ms,
        }))
    }

    /// Worktree icon for a repository addressed by path (no open session
    /// required); mirrors `worktree_icon` for the id-based lookup.
    pub async fn worktree_icon_for_path(
        &self,
        path: &Path,
    ) -> Result<Option<crate::icons::CachedIcon>> {
        let probe = crate::engines::gix::GixSession::discover(path)?;
        let root = probe
            .workdir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let key = format!("local:{}", root.display());
        let icon = self
            .scheduler
            .run(&CancellationToken::new(), move || {
                crate::engines::gix::worktree_icon::find_worktree_icon(&root)
            })
            .await?;
        Ok(icon.map(|icon| crate::icons::CachedIcon {
            key,
            data_url: crate::icons::encode_data_url(icon.content_type, &icon.bytes),
            content_type: icon.content_type.to_owned(),
            fetched_at_ms: icon.modified_ms,
        }))
    }

    pub async fn blame(
        &self,
        id: RepoId,
        query: BlameQuery,
        token: CancellationToken,
    ) -> Result<BlameResult> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&token, move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::blame::blame(&repo, &query)
            })
            .await
    }

    pub async fn list_branches_and_tags(
        &self,
        id: RepoId,
        token: CancellationToken,
    ) -> Result<RepoListing> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&token, move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                Ok(RepoListing {
                    branches: mutations::list_branches(&repo)?,
                    tags: mutations::list_tags(&repo)?,
                })
            })
            .await
    }

    pub async fn list_worktrees(&self, id: RepoId) -> Result<Vec<WorktreeInfo>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::worktrees::list_worktrees(&repo)
            })
            .await
    }

    pub async fn create_worktree(
        &self,
        id: RepoId,
        request: WorktreeCreateRequest,
        expected_generation: Option<Generation>,
    ) -> Result<WorktreeInfo> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::worktrees::create_worktree(repo, &request)
        })
        .await
    }

    pub async fn remove_worktree(
        &self,
        id: RepoId,
        request: WorktreeRemoveRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::worktrees::remove_worktree(repo, &request.name, request.force)
        })
        .await
    }

    pub async fn lock_worktree(
        &self,
        id: RepoId,
        request: WorktreeLockRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::worktrees::lock_worktree(
                repo,
                &request.name,
                request.reason.as_deref().unwrap_or(""),
            )
        })
        .await
    }

    pub async fn unlock_worktree(
        &self,
        id: RepoId,
        name: String,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::worktrees::unlock_worktree(repo, &name)
        })
        .await
    }

    pub async fn list_submodules(&self, id: RepoId) -> Result<Vec<SubmoduleInfo>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::submodules::list_submodules(&repo)
            })
            .await
    }

    pub async fn add_submodule(
        &self,
        id: RepoId,
        request: SubmoduleAddRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::submodules::add_submodule(repo, &request)
        })
        .await
    }

    pub async fn update_submodules(
        &self,
        id: RepoId,
        request: SubmoduleUpdateRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::submodules::update_submodule(repo, &request)
        })
        .await
    }

    pub async fn lfs_status(&self, id: RepoId) -> Result<LfsStatus> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::lfs::lfs_status(&repo)
            })
            .await
    }

    pub async fn lfs_smudge(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
    ) -> Result<usize> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::lfs::lfs_smudge(repo)
        })
        .await
    }

    /// Discovers commit-pipeline hook scripts for manual runs; read-only.
    pub async fn list_commit_hooks(&self, id: RepoId) -> Result<Vec<HookInfo>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                git2_hooks::list_commit_hooks(&repo)
            })
            .await
    }

    /// Executes one commit hook with captured output. Not routed through
    /// `run_write`: hooks mutate outside our index/generation model, so no
    /// generation bump is published and the frontend refreshes status on
    /// completion itself.
    pub async fn run_commit_hook(&self, id: RepoId, hook: String) -> Result<HookRunResult> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                git2_hooks::run_commit_hook(&repo, &hook)
            })
            .await
    }

    /// Reads a commit hook script's contents; missing scripts come back
    /// `exists: false` with empty content so the editor can create them.
    pub async fn read_commit_hook(&self, id: RepoId, hook: String) -> Result<HookContent> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                git2_hooks::read_commit_hook(&repo, &hook)
            })
            .await
    }

    /// Creates or overwrites a commit hook script. Like `run_commit_hook`,
    /// hook files live outside the index/generation model, so no generation
    /// bump is published; the frontend invalidates the hooks list itself.
    pub async fn write_commit_hook(&self, id: RepoId, hook: String, content: String) -> Result<()> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                git2_hooks::write_commit_hook(&repo, &hook, &content)
            })
            .await
    }

    pub async fn stage_paths(
        &self,
        id: RepoId,
        request: StageRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::stage(repo, &request.paths, request.all)
        })
        .await
    }

    pub async fn unstage_paths(
        &self,
        id: RepoId,
        paths: Vec<String>,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::unstage(repo, &paths)
        })
        .await
    }

    pub async fn discard_changes(
        &self,
        id: RepoId,
        request: DiscardRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::discard(repo, &request.paths, request.all)
        })
        .await
    }

    pub async fn commit(
        &self,
        id: RepoId,
        request: CommitRequest,
        expected_generation: Option<Generation>,
    ) -> Result<CommitExecution> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::commit(repo, &request)
                .map(|(summary, hook_runs)| CommitExecution { summary, hook_runs })
        })
        .await
    }

    pub async fn amend_commit(
        &self,
        id: RepoId,
        request: AmendRequest,
        expected_generation: Option<Generation>,
    ) -> Result<CommitSummary> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::amend(repo, &request)
        })
        .await
    }

    pub async fn checkout(
        &self,
        id: RepoId,
        request: CheckoutRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::checkout(repo, &request)
        })
        .await
    }

    pub async fn reset(
        &self,
        id: RepoId,
        request: ResetRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::reset(repo, &request)
        })
        .await
    }

    pub async fn create_branch(
        &self,
        id: RepoId,
        request: BranchCreateRequest,
        expected_generation: Option<Generation>,
    ) -> Result<BranchInfo> {
        self.run_write(id, expected_generation, move |repo| {
            mutations::create_branch(repo, &request)
        })
        .await
    }

    pub async fn delete_branch(
        &self,
        id: RepoId,
        name: String,
        force: bool,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        let name = crate::domain::BranchName::parse(&name, "branch")?;
        self.run_write(id, expected_generation, move |repo| {
            mutations::delete_branch(repo, &name, force)
        })
        .await
    }

    pub async fn rename_branch(
        &self,
        id: RepoId,
        old_name: String,
        new_name: String,
        force: bool,
        expected_generation: Option<Generation>,
    ) -> Result<BranchInfo> {
        let old = crate::domain::BranchName::parse(&old_name, "branch")?;
        let new = crate::domain::BranchName::parse(&new_name, "branch")?;
        self.run_write(id, expected_generation, move |repo| {
            mutations::rename_branch(repo, &old, &new, force)
        })
        .await
    }

    pub async fn create_tag(
        &self,
        id: RepoId,
        request: TagCreateRequest,
        expected_generation: Option<Generation>,
    ) -> Result<TagInfo> {
        let tag = self
            .run_write(id, expected_generation, move |repo| {
                mutations::create_tag(repo, &request)
            })
            .await?;
        // Summaries bake in tag names and the walk cache holds the tag map;
        // both outlive generation bumps by design, so drop them on tag
        // mutations to surface a new tag immediately instead of on reload.
        self.clear_repository_history_caches(id)?;
        Ok(tag)
    }

    pub async fn delete_tag(
        &self,
        id: RepoId,
        name: String,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        let name = crate::domain::TagName::parse(&name, "tag")?;
        self.run_write(id, expected_generation, move |repo| {
            mutations::delete_tag(repo, &name)
        })
        .await?;
        self.clear_repository_history_caches(id)?;
        Ok(())
    }

    pub async fn merge(
        &self,
        id: RepoId,
        request: MergeRequest,
        expected_generation: Option<Generation>,
        token: CancellationToken,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write_with_token(id, expected_generation, token, move |repo| {
            crate::engines::git2::workflows::merge(repo, &request)
        })
        .await
    }

    pub async fn merge_continue(
        &self,
        id: RepoId,
        request: MergeContinueRequest,
        expected_generation: Option<Generation>,
        token: CancellationToken,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write_with_token(id, expected_generation, token, move |repo| {
            crate::engines::git2::workflows::merge_continue(repo, &request)
        })
        .await
    }

    pub async fn merge_abort(
        &self,
        id: RepoId,
        _request: MergeAbortRequest,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::merge_abort(repo)
        })
        .await
    }

    pub async fn start_rebase(
        &self,
        id: RepoId,
        upstream: crate::domain::RevisionSpec,
        onto: Option<crate::domain::RevisionSpec>,
        expected_generation: Option<Generation>,
        token: CancellationToken,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write_with_token(id, expected_generation, token, move |repo| {
            crate::engines::git2::workflows::start_rebase(repo, &upstream, onto.as_ref())
        })
        .await
    }

    pub async fn continue_rebase(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
        token: CancellationToken,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write_with_token(id, expected_generation, token, move |repo| {
            crate::engines::git2::workflows::continue_rebase(repo)
        })
        .await
    }

    pub async fn abort_rebase(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::abort_rebase(repo)
        })
        .await
    }

    pub async fn cherry_pick(
        &self,
        id: RepoId,
        target: crate::domain::RevisionSpec,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::cherry_pick(repo, &target)
        })
        .await
    }

    pub async fn revert(
        &self,
        id: RepoId,
        request: RevertRequest,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::revert(repo, &request)
        })
        .await
    }

    pub async fn operation_state(&self, id: RepoId) -> Result<OperationState> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::workflows::operation_state(&repo)
            })
            .await
    }

    pub async fn resolve_conflict(
        &self,
        id: RepoId,
        request: ResolveConflictRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::resolve_conflict(repo, &request)
        })
        .await
    }

    pub async fn conflict_file(
        &self,
        id: RepoId,
        path: String,
        stage: u8,
        token: CancellationToken,
    ) -> Result<ConflictFile> {
        let entry = self.registry.get(id)?;
        let repo_path = entry.canonical_path.clone();
        self.scheduler
            .run(&token, move || {
                let repo = git2::Repository::discover(&repo_path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&repo_path, e))?;
                crate::engines::git2::workflows::conflict_file(&repo, &path, stage)
            })
            .await
    }

    pub async fn pick_continue(
        &self,
        id: RepoId,
        state_file: String,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::pick_continue(repo, &state_file)
        })
        .await
    }

    pub async fn pick_abort(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::pick_abort(repo)
        })
        .await
    }

    pub async fn stash_push(
        &self,
        id: RepoId,
        request: StashPushRequest,
        expected_generation: Option<Generation>,
    ) -> Result<ObjectId> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::stash_push(repo, &request)
        })
        .await
    }

    pub async fn stash_pop(
        &self,
        id: RepoId,
        request: StashPopRequest,
        expected_generation: Option<Generation>,
    ) -> Result<Vec<crate::engines::git2::workflows::StashEntry>> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::workflows::stash_pop(repo, &request)?;
            crate::engines::git2::workflows::stash_list(repo)
        })
        .await
    }

    pub async fn stash_list(
        &self,
        id: RepoId,
    ) -> Result<Vec<crate::engines::git2::workflows::StashEntry>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let mut repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::workflows::stash_list(&mut repo)
            })
            .await
    }

    pub async fn fetch(
        &self,
        id: RepoId,
        mut request: FetchRequest,
        token: CancellationToken,
    ) -> Result<()> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        let token_for_job = token.clone();
        let github = Arc::clone(&self.github);
        self.scheduler
            .run(&token, move || {
                let mut repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                if request.credential.is_none() {
                    request.credential = github.credential_for_remote(&repo, &request.remote);
                }
                crate::engines::git2::remotes::fetch(&mut repo, &request, &token_for_job)
            })
            .await?;
        let generation = entry.bump_generation();
        self.registry.hub().publish(id.0, generation.0, "fetch");
        Ok(())
    }

    pub async fn push(
        &self,
        id: RepoId,
        mut request: PushRequest,
        token: CancellationToken,
    ) -> Result<Vec<PushOutcome>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        let token_for_job = token.clone();
        let github = Arc::clone(&self.github);
        let outcomes = self
            .scheduler
            .run(&token, move || {
                let mut repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                if request.credential.is_none() {
                    request.credential = github.credential_for_remote(&repo, &request.remote);
                }
                crate::engines::git2::remotes::push(&mut repo, &request, &token_for_job)
            })
            .await?;
        let generation = entry.bump_generation();
        self.registry.hub().publish(id.0, generation.0, "push");
        Ok(outcomes)
    }

    pub async fn pull(
        &self,
        id: RepoId,
        request: PullRequest,
        expected_generation: Option<Generation>,
        token: CancellationToken,
    ) -> Result<crate::engines::git2::workflows::WorkflowOutcome> {
        let github = Arc::clone(&self.github);
        self.run_write_with_token(id, expected_generation, token.clone(), move |repo| {
            let mut fetch_request = FetchRequest {
                remote: request.remote.clone(),
                credential: request.credential.clone(),
                ..Default::default()
            };
            if fetch_request.credential.is_none() {
                fetch_request.credential =
                    github.credential_for_remote(repo, &fetch_request.remote);
            }
            crate::engines::git2::remotes::fetch(repo, &fetch_request, &token)?;
            let plan = crate::engines::git2::remotes::resolve_pull_upstream(repo, &request)?;
            let outcome = if request.rebase {
                crate::engines::git2::workflows::start_rebase(repo, &plan.upstream_spec, None)?
            } else {
                let outcome = crate::engines::git2::workflows::merge(
                    repo,
                    &MergeRequest {
                        target: plan.upstream_spec,
                        fast_forward_only: request.fast_forward_only,
                        no_fast_forward: false,
                        message: None,
                    },
                )?;
                // Automatic submodule + LFS integration after a merge.
                crate::engines::git2::submodules::integrate(repo);
                outcome
            };
            Ok(outcome)
        })
        .await
    }

    pub async fn add_remote(
        &self,
        id: RepoId,
        request: RemoteAddRequest,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::remotes::add(repo, &request)
        })
        .await
    }

    pub async fn remove_remote(
        &self,
        id: RepoId,
        name: String,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::remotes::remove(repo, &name)
        })
        .await
    }

    pub async fn set_remote_url(
        &self,
        id: RepoId,
        name: String,
        url: String,
        expected_generation: Option<Generation>,
    ) -> Result<()> {
        self.run_write(id, expected_generation, move |repo| {
            crate::engines::git2::remotes::set_url(repo, &name, &url)
        })
        .await
    }

    pub async fn list_remotes(&self, id: RepoId) -> Result<Vec<crate::api::remotes::RemoteInfo>> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::remotes::list(&repo)
            })
            .await
    }

    /// Lists remotes for a repository addressed by path rather than an open
    /// session id, so the sidebar and home page can show owner/labels even
    /// when no tab holds the repo open.
    pub async fn list_remotes_for_path(
        &self,
        path: &Path,
    ) -> Result<Vec<crate::api::remotes::RemoteInfo>> {
        let path = path.to_path_buf();
        self.scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                crate::engines::git2::remotes::list(&repo)
            })
            .await
    }

    pub async fn remote_repo_info(
        &self,
        id: RepoId,
    ) -> Result<crate::api::remote_info::RemoteRepoInfo> {
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();
        let remotes = self.list_remotes(id).await.unwrap_or_default();
        Ok(self.finalize_remote_info(&path, origin_url(&remotes)).await)
    }

    /// Remote info for a repository addressed by its durable path, so the
    /// sidebar and home page can show metadata even when no tab has an open
    /// session for the repo. Shares the disk cache with `remote_repo_info`.
    pub async fn remote_repo_info_for_path(
        &self,
        path: &Path,
    ) -> Result<crate::api::remote_info::RemoteRepoInfo> {
        let remotes = self.list_remotes_for_path(path).await.unwrap_or_default();
        Ok(self.finalize_remote_info(path, origin_url(&remotes)).await)
    }

    /// Applies README fallback and source labeling shared by the id- and
    /// path-based remote info lookups.
    async fn finalize_remote_info(
        &self,
        path: &Path,
        remote_url: Option<String>,
    ) -> crate::api::remote_info::RemoteRepoInfo {
        let mut result = match remote_url.as_deref().and_then(remote_repo_coords) {
            Some(coords) => self.cached_remote_info(coords).await,
            None => crate::api::remote_info::RemoteRepoInfo::default(),
        };
        // Fallback / override with local README if no description from remote.
        if result.description.is_none() || result.description.as_ref().unwrap().is_empty() {
            let readme_desc = try_read_readme(path);
            if result.source.as_deref() != Some("remote") && !readme_desc.is_empty() {
                result.description = Some(readme_desc);
                result.source = Some("readme".to_owned());
            }
        }
        if result.source.is_none()
            && (result.description.is_some() || result.stars.is_some() || result.forks.is_some())
        {
            result.source = Some("remote".to_owned());
        }
        if result.source.is_none() && result.description.is_none() {
            result.source = Some("none".to_owned());
        }
        // Language shares come from the local worktree, not the provider,
        // so they never hit the remote-info cache and are worth caching in
        // memory: walking a large worktree is the expensive part.
        result.languages = self.cached_language_stats(path).await;
        result.fetched_at_ms = Some(result.fetched_at_ms.unwrap_or_else(now_ms));
        result
    }

    /// Language shares keyed by repo path, bounded FIFO cache; the backend
    /// is a process singleton, so one map covers all callers.
    async fn cached_language_stats(
        &self,
        path: &Path,
    ) -> Option<Vec<crate::engines::gix::language_stats::LanguageEntry>> {
        {
            let cache = self.language_stats_cache.lock().unwrap();
            if let Some((_, cached)) = cache.iter().find(|(key, _)| key == path) {
                return cached.clone();
            }
        }
        let compute_path = path.to_path_buf();
        let stats = self
            .scheduler
            .run(&CancellationToken::new(), move || {
                crate::engines::gix::language_stats::language_stats(&compute_path)
            })
            .await;
        let entries = stats
            .ok()
            .filter(|stats| !stats.entries.is_empty())
            .map(|stats| stats.entries);
        let mut cache = self.language_stats_cache.lock().unwrap();
        if let Some(slot) = cache.iter_mut().find(|(key, _)| key == path) {
            slot.1 = entries.clone();
        } else {
            if cache.len() >= LANGUAGE_STATS_CACHE_CAP {
                cache.remove(0);
            }
            cache.push((path.to_path_buf(), entries.clone()));
        }
        entries
    }

    /// Resolves provider info through the disk cache: fresh entries are
    /// served directly, stale entries immediately while a background task
    /// revalidates, misses fetch synchronously. Only successful remote
    /// payloads are cached, so README-derived descriptions stay local.
    async fn cached_remote_info(
        &self,
        coords: RemoteRepoCoords,
    ) -> crate::api::remote_info::RemoteRepoInfo {
        let key = coords.cache_key();
        let dir = self.remote_info_cache.lock().unwrap().dir.clone();
        let Some(dir) = dir else {
            return fetch_remote_info(&coords).await.unwrap_or_default();
        };

        match read_cached_remote_info(&dir, &key) {
            Some((info, false)) => {
                touch_cache_file(&remote_info_cache_path(&dir, &key));
                info
            }
            Some((info, true)) => {
                touch_cache_file(&remote_info_cache_path(&dir, &key));
                self.spawn_remote_info_refresh(dir, &key, coords);
                info
            }
            None => match fetch_remote_info(&coords).await {
                Some(info) => {
                    store_remote_info(&dir, &key, &info);
                    info
                }
                None => crate::api::remote_info::RemoteRepoInfo::default(),
            },
        }
    }

    /// Serves stale bytes while one background task revalidates; concurrent
    /// refreshes of the same key are deduped until it finishes.
    fn spawn_remote_info_refresh(&self, dir: PathBuf, key: &str, coords: RemoteRepoCoords) {
        let key = key.to_owned();
        {
            let mut state = self.remote_info_cache.lock().unwrap();
            if !state.refreshing.insert(key.clone()) {
                return;
            }
        }
        let state = Arc::clone(&self.remote_info_cache);
        tokio::spawn(async move {
            if let Some(info) = fetch_remote_info(&coords).await {
                store_remote_info(&dir, &key, &info);
            }
            state.lock().unwrap().refreshing.remove(&key);
        });
    }

    /// Starts a clone and streams progress on the returned channel until a
    /// terminal event. Resolves with the operation id immediately; the
    /// download itself runs detached on the blocking scheduler so cancel
    /// works while the caller holds the receiver.
    pub async fn open_clone(
        &self,
        mut request: CloneRequest,
        token: CancellationToken,
    ) -> Result<(OperationId, mpsc::Receiver<CloneEvent>)> {
        token.check()?;
        let operation_id = OperationId(self.next_operation_id.fetch_add(1, Ordering::AcqRel));
        let operation = Arc::new(CloneOperation::new(operation_id, token.clone()));
        self.clone_operations.insert(operation.clone());

        if request.credential.is_none() {
            request.credential = self.github.credential_for_url(&request.url);
        }
        let (tx, rx) = mpsc::channel::<CloneEvent>(256);

        let op_for_job = operation.clone();
        let scheduler = self.scheduler.clone();
        let token_for_job = token.clone();
        let terminal_sent = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let terminal_sent_for_job = terminal_sent.clone();
        let tx_for_job = tx.clone();

        // The clone runs on a dedicated thread with its own runtime so its
        // lifetime never depends on which ambient runtime called
        // `open_clone` (tests use throwaway runtimes).
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("clone worker runtime");
            runtime.block_on(async move {
                let _ = scheduler
                    .run(&token, move || {
                        // Every send marks the flag so the tail guard knows
                        // the job reached a terminal state.
                        let emit = |event: CloneEvent, sent: &std::sync::atomic::AtomicBool| {
                            sent.store(true, Ordering::Release);
                            let _ = tx_for_job.blocking_send(event);
                        };
                        let mark = |sent: &std::sync::atomic::AtomicBool| {
                            sent.store(true, Ordering::Release);
                        };
                        let mut forward = |progress: CloneProgress| {
                            mark(&terminal_sent_for_job);
                            let _ = tx_for_job.blocking_send(CloneEvent::Progress {
                                operation_id: operation_id.0,
                                progress,
                            });
                        };
                        match crate::engines::git2::remotes::clone(
                            &request,
                            &token_for_job,
                            &mut forward,
                        ) {
                            Ok(path) => {
                                if let Ok(discovered) = git2::Repository::discover(&path) {
                                    crate::engines::git2::submodules::integrate(&discovered);
                                }
                                emit(
                                    CloneEvent::Completed {
                                        operation_id: operation_id.0,
                                        repo_path: path.to_string_lossy().into_owned(),
                                    },
                                    &terminal_sent_for_job,
                                );
                                Ok(())
                            }
                            Err(error) => {
                                if error.code() == "cancelled" || token_for_job.is_cancelled() {
                                    emit(
                                        CloneEvent::Cancelled {
                                            operation_id: operation_id.0,
                                        },
                                        &terminal_sent_for_job,
                                    );
                                } else {
                                    emit(
                                        CloneEvent::Failed {
                                            operation_id: operation_id.0,
                                            code: error.code().to_owned(),
                                            message: error.to_string(),
                                        },
                                        &terminal_sent_for_job,
                                    );
                                }
                                Err(error)
                            }
                        }
                    })
                    .await;

                // If the job never ran (cancelled before acquiring a permit),
                // no terminal event was queued; synthesize one so the stream
                // always terminates coherently instead of just closing.
                if !terminal_sent.load(Ordering::Acquire) && !token.is_cancelled() {
                    let _ = tx
                        .send(CloneEvent::Failed {
                            operation_id: operation_id.0,
                            code: "internal".into(),
                            message: "clone job ended without reporting a result".into(),
                        })
                        .await;
                } else if !terminal_sent.load(Ordering::Acquire) {
                    let _ = tx
                        .send(CloneEvent::Cancelled {
                            operation_id: operation_id.0,
                        })
                        .await;
                }

                drop(tx);
                op_for_job.mark_complete();
            });
        });

        self.clone_operations
            .prune_completed(self.config.keep_completed_operations);
        Ok((operation_id, rx))
    }

    pub async fn create_repository(
        &self,
        request: CreateRepositoryRequest,
    ) -> Result<CreateRepositoryResult> {
        let target = self
            .scheduler
            .run(&CancellationToken::new(), move || {
                scaffold_repository(&request)
            })
            .await?;
        let repo = self.open_repository(&target).await?;
        Ok(CreateRepositoryResult {
            path: target.to_string_lossy().into_owned(),
            repo,
        })
    }

    pub fn list_gitignore_templates(&self) -> Vec<GitignoreTemplateInfo> {
        templates::GITIGNORE_TEMPLATES
            .iter()
            .map(|t| GitignoreTemplateInfo {
                id: t.id.to_owned(),
                label: t.label.to_owned(),
            })
            .collect()
    }

    pub fn list_licenses(&self) -> Vec<LicenseTemplateInfo> {
        templates::LICENSE_TEMPLATES
            .iter()
            .map(|t| LicenseTemplateInfo {
                id: t.id.to_owned(),
                name: t.name.to_owned(),
                description: t.description.to_owned(),
            })
            .collect()
    }

    /// The connected GitHub account, `None` while signed out.
    pub fn github_account(&self) -> Option<AccountProfile> {
        self.github.account()
    }

    /// Starts a device-flow sign-in: returns the short code the user types
    /// into the browser. The flow then waits in [`Backend::github_complete_sign_in`].
    pub async fn github_begin_sign_in(&self) -> Result<DeviceFlowStart> {
        self.github.begin_sign_in().await
    }

    /// Polls until the user finishes authorization (or the flow is
    /// cancelled/expired) and resolves with the fresh profile.
    pub async fn github_complete_sign_in(&self) -> Result<AccountProfile> {
        self.github.complete_sign_in().await
    }

    /// Aborts an in-flight sign-in; the pending complete call resolves with
    /// a cancellation error.
    pub fn github_cancel_sign_in(&self) {
        self.github.cancel_sign_in();
    }

    pub async fn github_sign_out(&self) -> Result<()> {
        self.github.sign_out().await
    }

    /// Organizations of the signed-in user, for publish-target selection.
    pub async fn github_list_orgs(&self) -> Result<Vec<GithubOrg>> {
        self.github.list_orgs().await
    }

    /// One page of all GitHub notification threads (read + unread),
    /// newest first. Pages are 1-based.
    pub async fn github_list_notifications(&self, page: u32) -> Result<NotificationPage> {
        self.github.list_notifications(page).await
    }

    pub async fn github_mark_notification_read(&self, thread_id: String) -> Result<()> {
        self.github.mark_notification_read(&thread_id).await
    }

    pub async fn github_mark_all_notifications_read(&self) -> Result<()> {
        self.github.mark_all_notifications_read().await
    }

    /// Resolves a notification subject API URL to its web URL. Returns
    /// `None` when the subject is gone; the UI falls back to the repo.
    pub async fn github_resolve_subject_url(&self, subject_url: String) -> Result<Option<String>> {
        self.github.resolve_subject_url(&subject_url).await
    }

    /// Issues of a GitHub repository, `state` is open/closed/all.
    /// Pull requests are excluded.
    pub async fn github_list_issues(
        &self,
        owner: String,
        repo: String,
        state: String,
        labels: Vec<String>,
    ) -> Result<Vec<GithubIssueListItem>> {
        self.github
            .list_issues(&owner, &repo, &state, &labels)
            .await
    }

    /// Search issues across all of GitHub matching
    /// `is:issue involves:@me sort:updated-desc`.
    pub async fn github_search_issues(&self, page: u32) -> Result<SearchIssuePage> {
        self.github.search_issues(page).await
    }

    pub async fn github_get_issue(
        &self,
        owner: String,
        repo: String,
        number: u64,
    ) -> Result<GithubIssueDetail> {
        self.github.get_issue(&owner, &repo, number).await
    }

    pub async fn github_list_issue_comments(
        &self,
        owner: String,
        repo: String,
        number: u64,
    ) -> Result<Vec<GithubIssueComment>> {
        self.github.list_issue_comments(&owner, &repo, number).await
    }

    pub async fn github_list_issue_events(
        &self,
        owner: String,
        repo: String,
        number: u64,
    ) -> Result<Vec<GithubIssueEvent>> {
        self.github.list_issue_events(&owner, &repo, number).await
    }

    pub async fn github_create_issue_comment(
        &self,
        owner: String,
        repo: String,
        number: u64,
        body: String,
    ) -> Result<GithubIssueComment> {
        if body.trim().is_empty() {
            return Err(GitError::invalid_input("comment body must not be empty"));
        }
        self.github
            .create_issue_comment(&owner, &repo, number, &body)
            .await
    }

    /// Partial issue update: state (open/closed), labels, assignees.
    pub async fn github_update_issue(
        &self,
        owner: String,
        repo: String,
        number: u64,
        body: UpdateIssueBody,
    ) -> Result<GithubIssueDetail> {
        self.github.update_issue(&owner, &repo, number, &body).await
    }

    pub async fn github_create_issue(
        &self,
        owner: String,
        repo: String,
        title: String,
        body: Option<String>,
        labels: Vec<String>,
    ) -> Result<GithubIssueDetail> {
        if title.trim().is_empty() {
            return Err(GitError::invalid_input("issue title must not be empty"));
        }
        self.github
            .create_issue(&owner, &repo, &title, body.as_deref(), &labels)
            .await
    }

    pub async fn github_update_issue_comment(
        &self,
        owner: String,
        repo: String,
        comment_id: u64,
        body: String,
    ) -> Result<GithubIssueComment> {
        if body.trim().is_empty() {
            return Err(GitError::invalid_input("comment body must not be empty"));
        }
        self.github
            .update_issue_comment(&owner, &repo, comment_id, &body)
            .await
    }

    pub async fn github_delete_issue_comment(
        &self,
        owner: String,
        repo: String,
        comment_id: u64,
    ) -> Result<()> {
        self.github
            .delete_issue_comment(&owner, &repo, comment_id)
            .await
    }

    /// The signed-in user's access on the repository, for gating
    /// comment moderation in the UI.
    pub async fn github_repo_permissions(
        &self,
        owner: String,
        repo: String,
    ) -> Result<GithubRepoPermissions> {
        self.github.repo_permissions(&owner, &repo).await
    }

    /// Highlights one markdown code fence with the active theme pair.
    /// Pure and infallible: unknown languages stay plain.
    pub fn highlight_code(&self, language: String, text: String) -> HighlightedSnippet {
        crate::api::highlight::highlight_code(&language, &text)
    }

    /// Publishes this repository: creates the GitHub repo under `owner`
    /// (personal account when `None`), points `origin` at it, and pushes
    /// the current branch with upstream tracking. Credentials are injected
    /// automatically for github.com remotes when signed in.
    pub async fn publish_repository(
        &self,
        id: RepoId,
        request: PublishRepositoryRequest,
        expected_generation: Option<Generation>,
    ) -> Result<PublishResult> {
        let name = request.name.trim().to_owned();
        if name.is_empty() {
            return Err(GitError::invalid_input("repository name must not be empty"));
        }
        let entry = self.registry.get(id)?;
        let path = entry.canonical_path.clone();

        // Publishing requires a branch to push: unborn or detached HEADs
        // have nothing meaningful to publish yet.
        let head_branch = self
            .scheduler
            .run(&CancellationToken::new(), move || {
                let repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                match repo.head() {
                    Ok(head) => {
                        if !head.is_branch() {
                            return Err(GitError::invalid_input(
                                "cannot publish from a detached HEAD",
                            ));
                        }
                        let name = head.shorthand().map_err(GitError::from)?;
                        Ok(name.to_owned())
                    }
                    Err(err) => {
                        if err.code() == git2::ErrorCode::UnbornBranch {
                            Err(GitError::invalid_input(
                                "repository has no commits yet; make an initial commit first",
                            ))
                        } else {
                            Err(err.into())
                        }
                    }
                }
            })
            .await?;

        let body = CreateRepoBody {
            name,
            description: request
                .description
                .map(|d| d.trim().to_owned())
                .filter(|d| !d.is_empty()),
            private: request.private,
        };
        let created = self
            .github
            .create_repository(request.owner.as_deref(), &body)
            .await?;

        // Prefer GitHub's own clone URL; fall back to constructing it from
        // the full name (the API always includes both in practice).
        let remote_url = created
            .clone_url
            .clone()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| format!("https://github.com/{}.git", created.full_name));
        let has_origin = self
            .list_remotes(id)
            .await?
            .iter()
            .any(|remote| remote.name == "origin");
        if has_origin {
            self.set_remote_url(id, "origin".into(), remote_url.clone(), expected_generation)
                .await?;
        } else {
            self.add_remote(
                id,
                crate::api::remotes::RemoteAddRequest {
                    name: "origin".into(),
                    url: remote_url,
                    fetch_refspec: None,
                },
                expected_generation,
            )
            .await?;
        }

        self.push(
            id,
            crate::api::remotes::PushRequest {
                remote: "origin".into(),
                refspecs: vec![head_branch.clone()],
                force: false,
                set_upstream: true,
                credential: None,
            },
            CancellationToken::new(),
        )
        .await?;

        Ok(PublishResult {
            full_name: created.full_name,
            html_url: created.html_url,
            default_branch: created.default_branch.or(Some(head_branch)),
        })
    }

    async fn run_write<T, F>(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
        job: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut git2::Repository) -> Result<T> + Send + 'static,
    {
        self.run_write_with_token(id, expected_generation, CancellationToken::new(), job)
            .await
    }

    async fn run_write_with_token<T, F>(
        &self,
        id: RepoId,
        expected_generation: Option<Generation>,
        token: CancellationToken,
        job: F,
    ) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut git2::Repository) -> Result<T> + Send + 'static,
    {
        let entry = self.registry.get(id)?;
        entry.generation().matches_expected(expected_generation)?;
        let guard = entry.write_lock.lock().await;
        let path = entry.canonical_path.clone();
        let result = self
            .scheduler
            .run(&token, move || {
                let mut repo = git2::Repository::discover(&path)
                    .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
                job(&mut repo)
            })
            .await;
        drop(guard);
        let value = match result {
            Ok(value) => value,
            Err(err) => return Err(err),
        };
        let generation = entry.bump_generation();
        self.registry.hub().publish(id.0, generation.0, "mutation");
        self.operations
            .prune_completed(self.config.keep_completed_operations);
        Ok(value)
    }
}

fn build_snapshot(entry: &Arc<crate::runtime::registry::RepoEntry>) -> Result<RepoSnapshot> {
    Ok(RepoSnapshot {
        id: entry.next_snapshot_id(),
        generation: entry.generation(),
        workdir: entry.gix.workdir().map(Path::to_path_buf),
        git_dir: entry.gix.git_dir().to_path_buf(),
        head: entry.gix.head_state()?,
        sha_kind: entry.gix.sha_kind(),
    })
}

fn commit_detail_blocking(
    path: PathBuf,
    query: CommitDetailQuery,
) -> Result<crate::domain::CommitDetail> {
    let repo = git2::Repository::discover(&path)
        .map_err(|e| crate::streaming::pipeline::map_open_error(&path, e))?;
    let commit = crate::streaming::pipeline::resolve_commit(&repo, &query.revision)?;
    let summary = mutations::summarize_commit(&repo, commit.id())?;

    let mut opts = crate::streaming::pipeline::build_options(&DiffRequest {
        comparison: crate::streaming::model::DiffComparison::WorkingTree,
        detect_renames: query.detect_renames,
        ..Default::default()
    })?;
    opts.include_untracked(false);
    opts.recurse_untracked_dirs(false);

    let parent_tree = commit.parent(0).ok().map(|p| p.tree()).transpose()?;
    let tree = commit.tree()?;
    let mut diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
    if query.detect_renames {
        let mut find = git2::DiffFindOptions::new();
        diff.find_similar(Some(&mut find))?;
    }

    use crate::domain::FileChangeStat;
    use crate::streaming::model::SectionKind;
    let sections = crate::streaming::pipeline::enumerate_sections(&diff)?;
    let mut files = Vec::with_capacity(sections.len());
    let mut total_additions = 0u64;
    let mut total_deletions = 0u64;

    for section in &sections {
        let mut additions = 0u32;
        let mut deletions = 0u32;
        let mut binary = section.binary;
        if let Some(patch) = git2::Patch::from_diff(&diff, section.section_id as usize)? {
            for hunk in 0..patch.num_hunks() {
                for line in 0..patch.num_lines_in_hunk(hunk)? {
                    let diff_line = patch.line_in_hunk(hunk, line)?;
                    match diff_line.origin() {
                        '+' => additions += 1,
                        '-' => deletions += 1,
                        _ => {}
                    }
                }
            }
        } else {
            binary = true;
        }
        total_additions += additions as u64;
        total_deletions += deletions as u64;
        files.push(FileChangeStat {
            path: section.path.clone(),
            old_path: section.old_path.clone(),
            kind: match section.kind {
                SectionKind::Added => ChangeKind::Added,
                SectionKind::Deleted => ChangeKind::Deleted,
                SectionKind::Modified => ChangeKind::Modified,
                SectionKind::Renamed => ChangeKind::Renamed,
                SectionKind::Copied => ChangeKind::Copied,
                SectionKind::TypeChanged => ChangeKind::TypeChanged,
                SectionKind::Conflicted => ChangeKind::Conflicted,
            },
            additions,
            deletions,
            binary,
            old_mode: None,
            new_mode: None,
            old_id: None,
            new_id: None,
        });
    }

    Ok(crate::domain::CommitDetail {
        summary,
        files,
        total_additions,
        total_deletions,
    })
}

fn try_read_readme(repo_path: &Path) -> String {
    let workdir = repo_path;
    let readme_names = [
        "README.md",
        "Readme.md",
        "readme.md",
        "README",
        "Readme",
        "readme",
    ];
    for name in &readme_names {
        let path = workdir.join(name);
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(paragraph) = crate::domain::readme::first_paragraph(
                &content,
                crate::domain::readme::README_DESCRIPTION_MAX_CHARS,
            ) {
                return paragraph;
            }
        }
    }
    String::new()
}

/// How long a cached remote info payload is served without revalidation.
const REMOTE_INFO_TTL: Duration = Duration::from_secs(24 * 60 * 60);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// The repo's primary remote URL: `origin` when present, else the first.
fn origin_url(remotes: &[crate::api::remotes::RemoteInfo]) -> Option<String> {
    remotes
        .iter()
        .find(|remote| remote.name == "origin")
        .or_else(|| remotes.first())
        .and_then(|remote| remote.url.clone())
}

/// `host/owner/repo` coordinates for providers with a public repo API,
/// derived from an https remote URL. Lowercased for stable cache keys.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteRepoCoords {
    host: String,
    owner: String,
    repo: String,
}

fn remote_repo_coords(url: &str) -> Option<RemoteRepoCoords> {
    let parsed = url.trim().parse::<reqwest::Url>().ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(host.as_str());
    if host != "github.com" && host != "gitlab.com" {
        return None;
    }
    let path = parsed
        .path()
        .trim_start_matches('/')
        .trim_end_matches(".git");
    let (owner, repo) = path.split_once('/')?;
    let repo = repo.trim_end_matches('/');
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(RemoteRepoCoords {
        host: host.to_owned(),
        owner: owner.to_ascii_lowercase(),
        repo: repo.to_ascii_lowercase(),
    })
}

impl RemoteRepoCoords {
    fn is_gitlab(&self) -> bool {
        self.host == "gitlab.com"
    }

    fn cache_key(&self) -> String {
        let sanitize = crate::runtime::disk::sanitize_component;
        format!(
            "{}/{}/{}",
            sanitize(&self.host),
            sanitize(&self.owner),
            sanitize(&self.repo)
        )
    }

    fn api_url(&self) -> String {
        if self.is_gitlab() {
            format!(
                "https://gitlab.com/api/v4/projects/{}%2F{}",
                self.owner, self.repo
            )
        } else {
            format!("https://api.github.com/repos/{}/{}", self.owner, self.repo)
        }
    }
}

/// Live provider lookup. `None` means the host is unknown, the request
/// failed, or the response was unusable; callers fall back or keep stale
/// bytes instead of caching anything.
async fn fetch_remote_info(coords: &RemoteRepoCoords) -> Option<RemoteRepoInfo> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("gitau")
        .build()
        .ok()?;
    let response = client.get(coords.api_url()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let text = response.text().await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;

    let mut info = RemoteRepoInfo::default();
    if let Some(description) = parsed
        .get("description")
        .and_then(|d| d.as_str())
        .filter(|d| !d.is_empty())
    {
        info.description = Some(description.to_owned());
        info.source = Some("remote".to_owned());
    }
    let stars_key = if coords.is_gitlab() {
        "star_count"
    } else {
        "stargazers_count"
    };
    info.stars = parsed.get(stars_key).and_then(|s| s.as_u64());
    info.forks = parsed.get("forks_count").and_then(|f| f.as_u64());
    info.fetched_at_ms = Some(now_ms());
    Some(info)
}

fn remote_info_cache_path(dir: &Path, key: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    for segment in key.split('/') {
        path.push(segment);
    }
    path.set_extension("json");
    path
}

/// `(info, is_stale)`; `None` when nothing usable is cached.
fn read_cached_remote_info(dir: &Path, key: &str) -> Option<(RemoteRepoInfo, bool)> {
    let raw = std::fs::read(remote_info_cache_path(dir, key)).ok()?;
    let info: RemoteRepoInfo = serde_json::from_slice(&raw).ok()?;
    let age = now_ms().saturating_sub(info.fetched_at_ms.unwrap_or(0));
    let stale = age > REMOTE_INFO_TTL.as_millis() as u64;
    Some((info, stale))
}

fn store_remote_info(dir: &Path, key: &str, info: &RemoteRepoInfo) {
    let path = remote_info_cache_path(dir, key);
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_vec(info) {
        let _ = crate::runtime::disk::write_atomic(&path, &json);
    }
}

/// Best-effort mtime bump marking a cache file as used. Ignored failures
/// only delay pruning, never break serving.
fn touch_cache_file(path: &Path) {
    if let Ok(handle) = std::fs::File::options().write(true).open(path) {
        let _ = handle.set_modified(SystemTime::now());
    }
}

/// How long an untouched remote-info payload is kept. Matches the icon
/// cache retention so both disk caches share one cleanup horizon.
pub const REMOTE_INFO_PRUNE_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Recursively deletes `*.json` cache files untouched for longer than
/// `max_age`, using mtime with a fetch-time fallback for filesystems
/// without reliable mtimes. Prunes empty directories on the way out and
/// returns the number of files removed.
fn prune_dir_older_than(dir: &Path, max_age: Duration) -> usize {
    let now_system = SystemTime::now();
    let now_ms_value = now_ms();
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            removed += prune_dir_older_than(&path, max_age);
            if std::fs::read_dir(&path)
                .map(|mut rest| rest.next().is_none())
                .unwrap_or(false)
            {
                let _ = std::fs::remove_dir(&path);
            }
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let recent = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|mtime| now_system.duration_since(mtime).unwrap_or(Duration::ZERO) <= max_age)
            .unwrap_or_else(|_| {
                std::fs::read(&path)
                    .ok()
                    .and_then(|raw| serde_json::from_slice::<RemoteRepoInfo>(&raw).ok())
                    .is_some_and(|info| {
                        now_ms_value.saturating_sub(info.fetched_at_ms.unwrap_or(0))
                            <= max_age.as_millis() as u64
                    })
            });
        if !recent && std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Creates the repository folder (or reuses `parent_directory` itself for
/// in-place init), runs `git init`, and writes the requested scaffolding
/// files (never commits). Returns the canonical path.
fn scaffold_repository(request: &CreateRepositoryRequest) -> Result<PathBuf> {
    let parent = PathBuf::from(&request.parent_directory);
    if !parent.is_dir() {
        return Err(GitError::invalid_input(format!(
            "parent directory does not exist: {}",
            request.parent_directory
        )));
    }
    let (target, name) = if request.init_in_place {
        let name = parent
            .file_name()
            .map(|raw| raw.to_string_lossy().into_owned())
            .unwrap_or_default();
        (parent, name)
    } else {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(GitError::invalid_input("repository name is empty"));
        }
        if name == "." || name == ".." || name.contains('/') || name.contains('\\') {
            return Err(GitError::invalid_input(format!(
                "invalid repository name: {name}"
            )));
        }
        let target = parent.join(name);
        if target.exists() {
            let empty = std::fs::read_dir(&target)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(true);
            if !empty {
                return Err(GitError::Conflict {
                    details: format!("directory is not empty: {}", target.display()),
                });
            }
        }
        std::fs::create_dir_all(&target)?;
        (target, name.to_owned())
    };

    // Re-initializing an existing repository is a no-op for git but would
    // surprise the user; refuse instead. Only checked for in-place init so
    // creating a (nested) repo folder inside a repository keeps working.
    if request.init_in_place && git2::Repository::discover(&target).is_ok() {
        return Err(GitError::Conflict {
            details: format!(
                "directory is already a git repository: {}",
                target.display()
            ),
        });
    }

    let repo = git2::Repository::init(&target).map_err(|e| GitError::Conflict {
        details: format!("git init failed: {}", e.message()),
    })?;
    drop(repo);

    let mut files: Vec<(&str, String)> = Vec::new();
    if request.readme {
        files.push(("README.md", format!("# {name}\n")));
    }
    if let Some(id) = request.gitignore_template.as_deref() {
        match templates::gitignore_by_id(id) {
            Some(t) => files.push((".gitignore", t.body.to_owned())),
            None => {
                return Err(GitError::invalid_input(format!(
                    "unknown gitignore template: {id}"
                )));
            }
        }
    }
    if let Some(id) = request.license.as_deref() {
        match templates::license_by_id(id) {
            Some(t) => files.push(("LICENSE", templates::render_license(t, current_year(), ""))),
            None => {
                return Err(GitError::invalid_input(format!(
                    "unknown license template: {id}"
                )));
            }
        }
    }
    for (file_name, content) in files {
        std::fs::write(target.join(file_name), content)?;
    }

    Ok(std::fs::canonicalize(&target).unwrap_or(target))
}

/// Current calendar year from the unix epoch; avoids pulling chrono's clock
/// feature into the backend for one copyright header.
fn current_year() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    year_for_days((secs / 86_400) as i64)
}

/// Calendar year containing `days_since_epoch`, consuming whole calendar
/// years from 1970 so leap boundaries stay exact.
fn year_for_days(days_since_epoch: i64) -> i32 {
    let mut remaining = days_since_epoch;
    let mut year = 1970i64;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let length = if leap { 366 } else { 365 };
        if remaining >= length {
            remaining -= length;
            year += 1;
        } else {
            break;
        }
    }
    year as i32
}

#[cfg(test)]
mod remote_info_tests {
    use super::*;

    #[test]
    fn coords_parse_github_and_gitlab_urls() {
        let github = remote_repo_coords("https://github.com/Facebook/React.git").unwrap();
        assert_eq!(github.host, "github.com");
        assert_eq!(github.owner, "facebook");
        assert_eq!(github.repo, "react");
        assert_eq!(
            github.api_url(),
            "https://api.github.com/repos/facebook/react"
        );
        assert_eq!(github.cache_key(), "github.com/facebook/react");

        let www = remote_repo_coords("https://www.gitlab.com/group/repo/").unwrap();
        assert_eq!(www.host, "gitlab.com");
        assert_eq!(www.owner, "group");
        assert_eq!(www.repo, "repo");
        assert_eq!(
            www.api_url(),
            "https://gitlab.com/api/v4/projects/group%2Frepo"
        );
    }

    #[test]
    fn coords_reject_unknown_hosts_and_shapes() {
        for bad in [
            "git@github.com:octo/repo.git",
            "/local/path/repo",
            "https://bitbucket.org/team/repo.git",
            "https://github.com/just-owner",
            "not a url",
        ] {
            assert!(
                remote_repo_coords(bad).is_none(),
                "`{bad}` should not parse"
            );
        }
    }

    #[test]
    fn cache_roundtrips_and_reports_staleness() {
        let dir = tempfile::tempdir().unwrap();
        let key = "github.com/octo/repo";

        assert!(read_cached_remote_info(dir.path(), key).is_none());

        let mut info = RemoteRepoInfo {
            description: Some("demo".to_owned()),
            stars: Some(7),
            fetched_at_ms: Some(now_ms()),
            ..Default::default()
        };
        store_remote_info(dir.path(), key, &info);

        let (cached, stale) = read_cached_remote_info(dir.path(), key).unwrap();
        assert!(!stale);
        assert_eq!(cached.description.as_deref(), Some("demo"));
        assert_eq!(cached.stars, Some(7));

        info.fetched_at_ms = Some(now_ms() - REMOTE_INFO_TTL.as_millis() as u64 - 1);
        store_remote_info(dir.path(), key, &info);
        let (_, stale) = read_cached_remote_info(dir.path(), key).unwrap();
        assert!(stale);
    }

    #[test]
    fn corrupt_cache_entries_read_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = remote_info_cache_path(dir.path(), "github.com/octo/repo");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not json").unwrap();
        assert!(read_cached_remote_info(dir.path(), "github.com/octo/repo").is_none());
    }

    #[test]
    fn prune_removes_only_untouched_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = RemoteRepoInfo {
            description: Some("fresh".to_owned()),
            fetched_at_ms: Some(now_ms()),
            ..Default::default()
        };
        let old = RemoteRepoInfo {
            description: Some("old".to_owned()),
            fetched_at_ms: Some(0),
            ..Default::default()
        };
        store_remote_info(dir.path(), "github.com/octo/fresh", &fresh);
        store_remote_info(dir.path(), "github.com/octo/old", &old);

        let old_path = remote_info_cache_path(dir.path(), "github.com/octo/old");
        let old_time = SystemTime::now() - REMOTE_INFO_PRUNE_AFTER - Duration::from_secs(60);
        std::fs::File::options()
            .write(true)
            .open(&old_path)
            .unwrap()
            .set_modified(old_time)
            .unwrap();

        let backend = Backend::default();
        backend.initialize_remote_info_cache(dir.path().to_path_buf());
        assert_eq!(backend.prune_remote_info_cache(REMOTE_INFO_PRUNE_AFTER), 1);
        assert!(read_cached_remote_info(dir.path(), "github.com/octo/fresh").is_some());
        assert!(read_cached_remote_info(dir.path(), "github.com/octo/old").is_none());
    }
}

#[cfg(test)]
mod create_tests {
    use super::*;

    #[test]
    fn year_for_days_matches_known_dates() {
        assert_eq!(year_for_days(0), 1970);
        assert_eq!(year_for_days(364), 1970);
        // 1970 has 365 days; the next day is 1971-01-01.
        assert_eq!(year_for_days(365), 1971);
        // 2000-01-01 is day 10957 (30 years, 7 of them leaps).
        assert_eq!(year_for_days(10_956), 1999);
        assert_eq!(year_for_days(10_957), 2000);
        // 2000 itself is a leap year and runs one day longer.
        assert_eq!(year_for_days(11_322), 2000);
        assert_eq!(year_for_days(11_323), 2001);
        // Century handling: 2100-01-01 is day 47482.
        assert_eq!(year_for_days(47_481), 2099);
        assert_eq!(year_for_days(47_482), 2100);
    }
}
