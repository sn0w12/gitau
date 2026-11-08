/**
 * GENERATED FILE - do not edit by hand.
 *
 * Source of truth: #[tauri::command] signatures and the generated protocol.
 * Regenerate with: cargo run -p contract --bin export_contract
 * Freshness is verified by cargo test (contract::tests::generated_contracts_are_fresh).
 */

import type { CommandArgs, CommandResults } from "../protocol";
import { invokeCommand } from "./invoke";

export const commands = {
    /**
     * Families installed on this machine, used as the choices for the font
     * settings. Enumeration touches every font file, so it is cached for the
     * life of the process.
     */
    fonts_list: () =>
        invokeCommand<CommandResults["fonts_list"]>("fonts_list"),
    /**
     * The connected account for one provider, `None` while signed out.
     */
    forge_account: (args: CommandArgs["forge_account"]) =>
        invokeCommand<CommandResults["forge_account"]>("forge_account", { args }),
    /**
     * Starts a device-flow sign-in and returns the short code to display.
     */
    forge_begin_sign_in: (args: CommandArgs["forge_begin_sign_in"]) =>
        invokeCommand<CommandResults["forge_begin_sign_in"]>("forge_begin_sign_in", { args }),
    /**
     * Aborts an in-flight sign-in; a pending complete call errors out.
     */
    forge_cancel_sign_in: () =>
        invokeCommand<CommandResults["forge_cancel_sign_in"]>("forge_cancel_sign_in"),
    /**
     * Waits for browser authorization and resolves with the fresh connection.
     * Long-running by design; cancel via `forge_cancel_sign_in`.
     */
    forge_complete_sign_in: () =>
        invokeCommand<CommandResults["forge_complete_sign_in"]>("forge_complete_sign_in"),
    /**
     * Every connection that currently holds a token.
     */
    forge_connections: () =>
        invokeCommand<CommandResults["forge_connections"]>("forge_connections"),
    forge_create_issue: (args: CommandArgs["forge_create_issue"]) =>
        invokeCommand<CommandResults["forge_create_issue"]>("forge_create_issue", { args }),
    forge_create_issue_comment: (args: CommandArgs["forge_create_issue_comment"]) =>
        invokeCommand<CommandResults["forge_create_issue_comment"]>("forge_create_issue_comment", { args }),
    forge_create_pull: (args: CommandArgs["forge_create_pull"]) =>
        invokeCommand<CommandResults["forge_create_pull"]>("forge_create_pull", { args }),
    forge_delete_issue_comment: (args: CommandArgs["forge_delete_issue_comment"]) =>
        invokeCommand<CommandResults["forge_delete_issue_comment"]>("forge_delete_issue_comment", { args }),
    /**
     * One check run with its output, for the results dialog.
     */
    forge_get_check_run: (args: CommandArgs["forge_get_check_run"]) =>
        invokeCommand<CommandResults["forge_get_check_run"]>("forge_get_check_run", { args }),
    /**
     * A run's job log, split per step, mirroring the provider's job view.
     */
    forge_get_check_run_log: (args: CommandArgs["forge_get_check_run_log"]) =>
        invokeCommand<CommandResults["forge_get_check_run_log"]>("forge_get_check_run_log", { args }),
    forge_get_issue: (args: CommandArgs["forge_get_issue"]) =>
        invokeCommand<CommandResults["forge_get_issue"]>("forge_get_issue", { args }),
    forge_get_pull: (args: CommandArgs["forge_get_pull"]) =>
        invokeCommand<CommandResults["forge_get_pull"]>("forge_get_pull", { args }),
    /**
     * CI check runs for a commit, which is a pull request's head sha.
     */
    forge_list_check_runs: (args: CommandArgs["forge_list_check_runs"]) =>
        invokeCommand<CommandResults["forge_list_check_runs"]>("forge_list_check_runs", { args }),
    /**
     * Commit statuses for a commit, one per reporter.
     */
    forge_list_commit_statuses: (args: CommandArgs["forge_list_commit_statuses"]) =>
        invokeCommand<CommandResults["forge_list_commit_statuses"]>("forge_list_commit_statuses", { args }),
    forge_list_issue_comments: (args: CommandArgs["forge_list_issue_comments"]) =>
        invokeCommand<CommandResults["forge_list_issue_comments"]>("forge_list_issue_comments", { args }),
    forge_list_issue_events: (args: CommandArgs["forge_list_issue_events"]) =>
        invokeCommand<CommandResults["forge_list_issue_events"]>("forge_list_issue_events", { args }),
    /**
     * Issues of a repository. `state` is open/closed/all; `labels` matches
     * issues carrying every named label.
     */
    forge_list_issues: (args: CommandArgs["forge_list_issues"]) =>
        invokeCommand<CommandResults["forge_list_issues"]>("forge_list_issues", { args }),
    forge_list_labels: (args: CommandArgs["forge_list_labels"]) =>
        invokeCommand<CommandResults["forge_list_labels"]>("forge_list_labels", { args }),
    /**
     * One page of notification threads (read + unread), newest first. The
     * frontend derives unread counts and filters from these pages. Pages are
     * 1-based, 100 threads each.
     */
    forge_list_notifications: (args: CommandArgs["forge_list_notifications"]) =>
        invokeCommand<CommandResults["forge_list_notifications"]>("forge_list_notifications", { args }),
    /**
     * Organizations the signed-in user can publish into.
     */
    forge_list_orgs: (args: CommandArgs["forge_list_orgs"]) =>
        invokeCommand<CommandResults["forge_list_orgs"]>("forge_list_orgs", { args }),
    /**
     * Commits on the pull request's head branch, oldest first.
     */
    forge_list_pull_commits: (args: CommandArgs["forge_list_pull_commits"]) =>
        invokeCommand<CommandResults["forge_list_pull_commits"]>("forge_list_pull_commits", { args }),
    /**
     * Pull requests of a repository. `state` is open/closed/all; `labels`
     * matches pull requests carrying every named label.
     */
    forge_list_pull_requests: (args: CommandArgs["forge_list_pull_requests"]) =>
        invokeCommand<CommandResults["forge_list_pull_requests"]>("forge_list_pull_requests", { args }),
    forge_list_pull_review_comments: (args: CommandArgs["forge_list_pull_review_comments"]) =>
        invokeCommand<CommandResults["forge_list_pull_review_comments"]>("forge_list_pull_review_comments", { args }),
    forge_list_pull_reviews: (args: CommandArgs["forge_list_pull_reviews"]) =>
        invokeCommand<CommandResults["forge_list_pull_reviews"]>("forge_list_pull_reviews", { args }),
    /**
     * Workflow runs for a commit, newest first.
     */
    forge_list_workflow_runs: (args: CommandArgs["forge_list_workflow_runs"]) =>
        invokeCommand<CommandResults["forge_list_workflow_runs"]>("forge_list_workflow_runs", { args }),
    /**
     * Marks every thread read.
     */
    forge_mark_all_notifications_read: (args: CommandArgs["forge_mark_all_notifications_read"]) =>
        invokeCommand<CommandResults["forge_mark_all_notifications_read"]>("forge_mark_all_notifications_read", { args }),
    /**
     * Marks one thread read.
     */
    forge_mark_notification_read: (args: CommandArgs["forge_mark_notification_read"]) =>
        invokeCommand<CommandResults["forge_mark_notification_read"]>("forge_mark_notification_read", { args }),
    forge_merge_pull: (args: CommandArgs["forge_merge_pull"]) =>
        invokeCommand<CommandResults["forge_merge_pull"]>("forge_merge_pull", { args }),
    /**
     * Providers this build can talk to, with their capabilities, for the UI to
     * gate views and build the provider picker.
     */
    forge_providers: () =>
        invokeCommand<CommandResults["forge_providers"]>("forge_providers"),
    /**
     * Creates the repository on the chosen provider, wires up `origin`, and
     * pushes the current branch with upstream tracking.
     */
    forge_publish_repository: (args: CommandArgs["forge_publish_repository"]) =>
        invokeCommand<CommandResults["forge_publish_repository"]>("forge_publish_repository", { args }),
    /**
     * The signed-in user's access on the repository, for gating comment
     * moderation in the UI.
     */
    forge_repo_permissions: (args: CommandArgs["forge_repo_permissions"]) =>
        invokeCommand<CommandResults["forge_repo_permissions"]>("forge_repo_permissions", { args }),
    /**
     * Resolves a notification subject API URL to its web URL, for types
     * without a static mapping. `None` when the subject is gone.
     */
    forge_resolve_subject_url: (args: CommandArgs["forge_resolve_subject_url"]) =>
        invokeCommand<CommandResults["forge_resolve_subject_url"]>("forge_resolve_subject_url", { args }),
    /**
     * Search issues across a provider matching `is:issue involves:@me`.
     */
    forge_search_issues: (args: CommandArgs["forge_search_issues"]) =>
        invokeCommand<CommandResults["forge_search_issues"]>("forge_search_issues", { args }),
    /**
     * Search pull requests across a provider matching `is:pr involves:@me`.
     */
    forge_search_pull_requests: (args: CommandArgs["forge_search_pull_requests"]) =>
        invokeCommand<CommandResults["forge_search_pull_requests"]>("forge_search_pull_requests", { args }),
    /**
     * Removes the stored token and profile cache for one connection.
     */
    forge_sign_out: (args: CommandArgs["forge_sign_out"]) =>
        invokeCommand<CommandResults["forge_sign_out"]>("forge_sign_out", { args }),
    /**
     * Partial issue update: `body` carries state (open/closed), description,
     * and replacement label and assignee name lists.
     */
    forge_update_issue: (args: CommandArgs["forge_update_issue"]) =>
        invokeCommand<CommandResults["forge_update_issue"]>("forge_update_issue", { args }),
    forge_update_issue_comment: (args: CommandArgs["forge_update_issue_comment"]) =>
        invokeCommand<CommandResults["forge_update_issue_comment"]>("forge_update_issue_comment", { args }),
    forge_update_pull: (args: CommandArgs["forge_update_pull"]) =>
        invokeCommand<CommandResults["forge_update_pull"]>("forge_update_pull", { args }),
    git_abort_rebase: (args: CommandArgs["git_abort_rebase"]) =>
        invokeCommand<CommandResults["git_abort_rebase"]>("git_abort_rebase", { args }),
    git_add_remote: (args: CommandArgs["git_add_remote"]) =>
        invokeCommand<CommandResults["git_add_remote"]>("git_add_remote", { args }),
    git_add_submodule: (args: CommandArgs["git_add_submodule"]) =>
        invokeCommand<CommandResults["git_add_submodule"]>("git_add_submodule", { args }),
    git_amend_commit: (args: CommandArgs["git_amend_commit"]) =>
        invokeCommand<CommandResults["git_amend_commit"]>("git_amend_commit", { args }),
    git_blame: (args: CommandArgs["git_blame"]) =>
        invokeCommand<CommandResults["git_blame"]>("git_blame", { args }),
    git_branch_ahead_behind: (args: CommandArgs["git_branch_ahead_behind"]) =>
        invokeCommand<CommandResults["git_branch_ahead_behind"]>("git_branch_ahead_behind", { args }),
    git_cancel_operation: (args: CommandArgs["git_cancel_operation"]) =>
        invokeCommand<CommandResults["git_cancel_operation"]>("git_cancel_operation", { args }),
    git_checkout: (args: CommandArgs["git_checkout"]) =>
        invokeCommand<CommandResults["git_checkout"]>("git_checkout", { args }),
    git_cherry_pick: (args: CommandArgs["git_cherry_pick"]) =>
        invokeCommand<CommandResults["git_cherry_pick"]>("git_cherry_pick", { args }),
    /**
     * Starts a clone and streams CloneEvent updates until a terminal event.
     * Resolves with the operation id, usable with `git_cancel_operation`.
     */
    git_clone: (args: CommandArgs["git_clone"]) =>
        invokeCommand<CommandResults["git_clone"]>("git_clone", { args }),
    git_close_repository: (args: CommandArgs["git_close_repository"]) =>
        invokeCommand<CommandResults["git_close_repository"]>("git_close_repository", { args }),
    git_commit: (args: CommandArgs["git_commit"]) =>
        invokeCommand<CommandResults["git_commit"]>("git_commit", { args }),
    git_commit_detail: (args: CommandArgs["git_commit_detail"]) =>
        invokeCommand<CommandResults["git_commit_detail"]>("git_commit_detail", { args }),
    git_conflict_file: (args: CommandArgs["git_conflict_file"]) =>
        invokeCommand<CommandResults["git_conflict_file"]>("git_conflict_file", { args }),
    git_continue_rebase: (args: CommandArgs["git_continue_rebase"]) =>
        invokeCommand<CommandResults["git_continue_rebase"]>("git_continue_rebase", { args }),
    git_create_branch: (args: CommandArgs["git_create_branch"]) =>
        invokeCommand<CommandResults["git_create_branch"]>("git_create_branch", { args }),
    git_create_tag: (args: CommandArgs["git_create_tag"]) =>
        invokeCommand<CommandResults["git_create_tag"]>("git_create_tag", { args }),
    git_create_worktree: (args: CommandArgs["git_create_worktree"]) =>
        invokeCommand<CommandResults["git_create_worktree"]>("git_create_worktree", { args }),
    git_delete_branch: (args: CommandArgs["git_delete_branch"]) =>
        invokeCommand<CommandResults["git_delete_branch"]>("git_delete_branch", { args }),
    git_delete_tag: (args: CommandArgs["git_delete_tag"]) =>
        invokeCommand<CommandResults["git_delete_tag"]>("git_delete_tag", { args }),
    git_discard_changes: (args: CommandArgs["git_discard_changes"]) =>
        invokeCommand<CommandResults["git_discard_changes"]>("git_discard_changes", { args }),
    git_fetch: (args: CommandArgs["git_fetch"]) =>
        invokeCommand<CommandResults["git_fetch"]>("git_fetch", { args }),
    git_file_at_revision: (args: CommandArgs["git_file_at_revision"]) =>
        invokeCommand<CommandResults["git_file_at_revision"]>("git_file_at_revision", { args }),
    git_history_chart: (args: CommandArgs["git_history_chart"]) =>
        invokeCommand<CommandResults["git_history_chart"]>("git_history_chart", { args }),
    git_history_page: (args: CommandArgs["git_history_page"]) =>
        invokeCommand<CommandResults["git_history_page"]>("git_history_page", { args }),
    git_init_repository: (args: CommandArgs["git_init_repository"]) =>
        invokeCommand<CommandResults["git_init_repository"]>("git_init_repository", { args }),
    git_lfs_smudge: (args: CommandArgs["git_lfs_smudge"]) =>
        invokeCommand<CommandResults["git_lfs_smudge"]>("git_lfs_smudge", { args }),
    git_lfs_status: (args: CommandArgs["git_lfs_status"]) =>
        invokeCommand<CommandResults["git_lfs_status"]>("git_lfs_status", { args }),
    git_list_branches_and_tags: (args: CommandArgs["git_list_branches_and_tags"]) =>
        invokeCommand<CommandResults["git_list_branches_and_tags"]>("git_list_branches_and_tags", { args }),
    git_list_commit_hooks: (args: CommandArgs["git_list_commit_hooks"]) =>
        invokeCommand<CommandResults["git_list_commit_hooks"]>("git_list_commit_hooks", { args }),
    git_list_gitignore_templates: () =>
        invokeCommand<CommandResults["git_list_gitignore_templates"]>("git_list_gitignore_templates"),
    git_list_licenses: () =>
        invokeCommand<CommandResults["git_list_licenses"]>("git_list_licenses"),
    git_list_remotes: (args: CommandArgs["git_list_remotes"]) =>
        invokeCommand<CommandResults["git_list_remotes"]>("git_list_remotes", { args }),
    /**
     * Lists remotes for a repository addressed by path, so metadata can be
     * shown without an open session.
     */
    git_list_remotes_by_path: (args: CommandArgs["git_list_remotes_by_path"]) =>
        invokeCommand<CommandResults["git_list_remotes_by_path"]>("git_list_remotes_by_path", { args }),
    git_list_submodules: (args: CommandArgs["git_list_submodules"]) =>
        invokeCommand<CommandResults["git_list_submodules"]>("git_list_submodules", { args }),
    git_list_worktrees: (args: CommandArgs["git_list_worktrees"]) =>
        invokeCommand<CommandResults["git_list_worktrees"]>("git_list_worktrees", { args }),
    git_lock_worktree: (args: CommandArgs["git_lock_worktree"]) =>
        invokeCommand<CommandResults["git_lock_worktree"]>("git_lock_worktree", { args }),
    git_merge: (args: CommandArgs["git_merge"]) =>
        invokeCommand<CommandResults["git_merge"]>("git_merge", { args }),
    git_merge_abort: (args: CommandArgs["git_merge_abort"]) =>
        invokeCommand<CommandResults["git_merge_abort"]>("git_merge_abort", { args }),
    git_merge_continue: (args: CommandArgs["git_merge_continue"]) =>
        invokeCommand<CommandResults["git_merge_continue"]>("git_merge_continue", { args }),
    git_open_diff: (args: CommandArgs["git_open_diff"]) =>
        invokeCommand<CommandResults["git_open_diff"]>("git_open_diff", { args }),
    git_open_graph: (args: CommandArgs["git_open_graph"]) =>
        invokeCommand<CommandResults["git_open_graph"]>("git_open_graph", { args }),
    git_open_repository: (args: CommandArgs["git_open_repository"]) =>
        invokeCommand<CommandResults["git_open_repository"]>("git_open_repository", { args }),
    git_operation_state: (args: CommandArgs["git_operation_state"]) =>
        invokeCommand<CommandResults["git_operation_state"]>("git_operation_state", { args }),
    git_pull: (args: CommandArgs["git_pull"]) =>
        invokeCommand<CommandResults["git_pull"]>("git_pull", { args }),
    git_push: (args: CommandArgs["git_push"]) =>
        invokeCommand<CommandResults["git_push"]>("git_push", { args }),
    git_read_commit_hook: (args: CommandArgs["git_read_commit_hook"]) =>
        invokeCommand<CommandResults["git_read_commit_hook"]>("git_read_commit_hook", { args }),
    git_read_diff_context: (args: CommandArgs["git_read_diff_context"]) =>
        invokeCommand<CommandResults["git_read_diff_context"]>("git_read_diff_context", { args }),
    git_read_diff_image: (args: CommandArgs["git_read_diff_image"]) =>
        invokeCommand<CommandResults["git_read_diff_image"]>("git_read_diff_image", { args }),
    git_read_diff_range: (args: CommandArgs["git_read_diff_range"]) =>
        invokeCommand<CommandResults["git_read_diff_range"]>("git_read_diff_range", { args }),
    git_read_graph_range: (args: CommandArgs["git_read_graph_range"]) =>
        invokeCommand<CommandResults["git_read_graph_range"]>("git_read_graph_range", { args }),
    git_read_worktree_image: (args: CommandArgs["git_read_worktree_image"]) =>
        invokeCommand<CommandResults["git_read_worktree_image"]>("git_read_worktree_image", { args }),
    git_readme: (args: CommandArgs["git_readme"]) =>
        invokeCommand<CommandResults["git_readme"]>("git_readme", { args }),
    git_remote_repo_info: (args: CommandArgs["git_remote_repo_info"]) =>
        invokeCommand<CommandResults["git_remote_repo_info"]>("git_remote_repo_info", { args }),
    /**
     * Remote info for a repository addressed by path; used by the sidebar and
     * home page so descriptions survive closing the last tab.
     */
    git_remote_repo_info_by_path: (args: CommandArgs["git_remote_repo_info_by_path"]) =>
        invokeCommand<CommandResults["git_remote_repo_info_by_path"]>("git_remote_repo_info_by_path", { args }),
    git_remove_remote: (args: CommandArgs["git_remove_remote"]) =>
        invokeCommand<CommandResults["git_remove_remote"]>("git_remove_remote", { args }),
    git_remove_repository: (args: CommandArgs["git_remove_repository"]) =>
        invokeCommand<CommandResults["git_remove_repository"]>("git_remove_repository", { args }),
    git_remove_worktree: (args: CommandArgs["git_remove_worktree"]) =>
        invokeCommand<CommandResults["git_remove_worktree"]>("git_remove_worktree", { args }),
    git_rename_branch: (args: CommandArgs["git_rename_branch"]) =>
        invokeCommand<CommandResults["git_rename_branch"]>("git_rename_branch", { args }),
    /**
     * Resolves the icon for an open repository. An icon file found in the
     * worktree (favicon/logo/icon names, gitignored paths excluded) wins;
     * otherwise the owner avatar of the origin remote is used. `None` means
     * the repo has no worktree icon and no provider supplies a remote icon.
     */
    git_repo_icon: (args: CommandArgs["git_repo_icon"]) =>
        invokeCommand<CommandResults["git_repo_icon"]>("git_repo_icon", { args }),
    /**
     * Resolves the icon for a repository addressed by path, mirroring
     * `git_repo_icon` but without requiring an open session.
     */
    git_repo_icon_by_path: (args: CommandArgs["git_repo_icon_by_path"]) =>
        invokeCommand<CommandResults["git_repo_icon_by_path"]>("git_repo_icon_by_path", { args }),
    git_repository_snapshot: (args: CommandArgs["git_repository_snapshot"]) =>
        invokeCommand<CommandResults["git_repository_snapshot"]>("git_repository_snapshot", { args }),
    git_reset: (args: CommandArgs["git_reset"]) =>
        invokeCommand<CommandResults["git_reset"]>("git_reset", { args }),
    git_resolve_conflict: (args: CommandArgs["git_resolve_conflict"]) =>
        invokeCommand<CommandResults["git_resolve_conflict"]>("git_resolve_conflict", { args }),
    git_revert: (args: CommandArgs["git_revert"]) =>
        invokeCommand<CommandResults["git_revert"]>("git_revert", { args }),
    /**
     * Runs a hook and streams its output: each line reaches `on_event` as the
     * script writes it, and the command resolves with the terminal result once
     * every line has been delivered.
     */
    git_run_commit_hook_streamed: (args: CommandArgs["git_run_commit_hook_streamed"]) =>
        invokeCommand<CommandResults["git_run_commit_hook_streamed"]>("git_run_commit_hook_streamed", { args }),
    git_set_remote_url: (args: CommandArgs["git_set_remote_url"]) =>
        invokeCommand<CommandResults["git_set_remote_url"]>("git_set_remote_url", { args }),
    git_stage_paths: (args: CommandArgs["git_stage_paths"]) =>
        invokeCommand<CommandResults["git_stage_paths"]>("git_stage_paths", { args }),
    git_start_rebase: (args: CommandArgs["git_start_rebase"]) =>
        invokeCommand<CommandResults["git_start_rebase"]>("git_start_rebase", { args }),
    git_stash_list: (args: CommandArgs["git_stash_list"]) =>
        invokeCommand<CommandResults["git_stash_list"]>("git_stash_list", { args }),
    git_stash_pop: (args: CommandArgs["git_stash_pop"]) =>
        invokeCommand<CommandResults["git_stash_pop"]>("git_stash_pop", { args }),
    git_stash_push: (args: CommandArgs["git_stash_push"]) =>
        invokeCommand<CommandResults["git_stash_push"]>("git_stash_push", { args }),
    git_status: (args: CommandArgs["git_status"]) =>
        invokeCommand<CommandResults["git_status"]>("git_status", { args }),
    git_unlock_worktree: (args: CommandArgs["git_unlock_worktree"]) =>
        invokeCommand<CommandResults["git_unlock_worktree"]>("git_unlock_worktree", { args }),
    git_unstage_paths: (args: CommandArgs["git_unstage_paths"]) =>
        invokeCommand<CommandResults["git_unstage_paths"]>("git_unstage_paths", { args }),
    git_update_submodules: (args: CommandArgs["git_update_submodules"]) =>
        invokeCommand<CommandResults["git_update_submodules"]>("git_update_submodules", { args }),
    git_write_commit_hook: (args: CommandArgs["git_write_commit_hook"]) =>
        invokeCommand<CommandResults["git_write_commit_hook"]>("git_write_commit_hook", { args }),
    /**
     * Highlights one markdown code fence with the active theme pair.
     * Pure and infallible: unknown languages stay plain.
     */
    highlight_code: (args: CommandArgs["highlight_code"]) =>
        invokeCommand<CommandResults["highlight_code"]>("highlight_code", { args }),
    /**
     * Forces a revalidation of one remote's icon right now, bypassing TTL.
     */
    icon_refresh: (args: CommandArgs["icon_refresh"]) =>
        invokeCommand<CommandResults["icon_refresh"]>("icon_refresh", { args }),
    /**
     * Resolves (and caches) the owner/org avatar for a git remote URL.
     * Returns a data URL ready for `<img src>`, or `None` when no forge
     * provider supplies an icon for the remote.
     */
    icon_resolve: (args: CommandArgs["icon_resolve"]) =>
        invokeCommand<CommandResults["icon_resolve"]>("icon_resolve", { args }),
    /**
     * Launches the editor configured in `editorCommand` with the given target.
     * `relative_path` joins onto `path` on the Rust side so Windows separators
     * resolve exactly; callers pass a repo root and a repo-relative file path.
     *
     * Resolution and spawning run on the blocking pool: a fallback that cannot
     * find the program consults the shell environment, which can take a beat.
     */
    open_in_editor: (args: CommandArgs["open_in_editor"]) =>
        invokeCommand<CommandResults["open_in_editor"]>("open_in_editor", { args }),
    open_in_terminal: (args: CommandArgs["open_in_terminal"]) =>
        invokeCommand<CommandResults["open_in_terminal"]>("open_in_terminal", { args }),
    /**
     * Reveals a path in the OS file manager: a directory opens in the file
     * manager, a file is revealed (selected) inside its parent. `relative_path`
     * joins onto `path` on the Rust side, mirroring `open_in_editor`.
     */
    reveal_in_file_manager: (args: CommandArgs["reveal_in_file_manager"]) =>
        invokeCommand<CommandResults["reveal_in_file_manager"]>("reveal_in_file_manager", { args }),
    /**
     * Loads the persisted app session (tabs, active tab, open repos).
     */
    session_load: () =>
        invokeCommand<CommandResults["session_load"]>("session_load"),
    session_repositories: (args: CommandArgs["session_repositories"]) =>
        invokeCommand<CommandResults["session_repositories"]>("session_repositories", { args }),
    /**
     * Persists the whole session document. The frontend owns its schema; the
     * backend only enforces the version field.
     */
    session_save: (args: CommandArgs["session_save"]) =>
        invokeCommand<CommandResults["session_save"]>("session_save", { args }),
    settings_load: () =>
        invokeCommand<CommandResults["settings_load"]>("settings_load"),
    settings_set: (args: CommandArgs["settings_set"]) =>
        invokeCommand<CommandResults["settings_set"]>("settings_set", { args }),
};
