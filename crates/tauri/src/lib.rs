mod commands;
pub mod session;
pub mod settings;
mod state;

use std::sync::Arc;

use state::AppState;
use tauri::Manager;

fn prevent_default() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_prevent_default::Flags;
    #[cfg(debug_assertions)]
    {
        tauri_plugin_prevent_default::Builder::new()
            .with_flags(Flags::empty())
            .build()
    }
    #[cfg(not(debug_assertions))]
    {
        tauri_plugin_prevent_default::Builder::new()
            .with_flags(Flags::CONTEXT_MENU)
            .build()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The webview stays hidden until it finishes loading, so the elapsed
    // logged there is the perceived launch time.
    let started_at = std::time::Instant::now();
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                // The watcher backend logs every filesystem event at debug,
                // which buries app output whenever the worktree is busy.
                .level_for("notify", log::LevelFilter::Warn)
                .level_for("notify_types", log::LevelFilter::Warn)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: None,
                    }),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(external_navigation_plugin())
        .plugin(prevent_default())
        .manage(AppState::default())
        .setup(move |app| {
            let handle = app.handle().clone();

            // Settings live in the platform config dir; if it is unavailable
            // the store keeps operating on in-memory defaults.
            let state = handle.state::<AppState>();
            match handle.path().app_config_dir() {
                Ok(config_dir) => {
                    state.settings.initialize(config_dir.join("settings.json"));
                    commands::settings::refresh_global_ignore(&state);
                }
                Err(error) => {
                    log::warn!("app config dir unavailable; settings stay in memory: {error}");
                }
            }

            // Remote icons and remote repo info caches under the platform
            // cache dir; without it both stay in-memory only.
            match handle.path().app_cache_dir() {
                Ok(cache_dir) => {
                    state.icons.initialize(cache_dir.join("remote-icons"));
                    state
                        .backend
                        .initialize_remote_info_cache(cache_dir.join("repo-info"));
                }
                Err(error) => {
                    log::warn!("app cache dir unavailable; icons stay in memory: {error}");
                }
            }

            // The whole UI session (tabs + open repos) lives beside settings.
            // `--session <path>` overrides it so the dev workspace can run
            // against a fixture file.
            if let Some(path) = crate::session::session_path_override() {
                // A missing override would silently start empty (the store
                // treats it as first run), which looks like the flag was
                // ignored. Say so loudly instead.
                if !path.exists() {
                    log::warn!(
                        "session override does not exist, starting empty: {}",
                        path.display()
                    );
                }
                log::info!("using session file override: {}", path.display());
                state.session.initialize(path);
            } else {
                match handle.path().app_config_dir() {
                    Ok(config_dir) => {
                        state.session.initialize(config_dir.join("session.json"));
                    }
                    Err(error) => {
                        log::warn!(
                            "app config dir unavailable; session stays in memory: {error}"
                        );
                    }
                }
            }

            // GitHub account profile cache beside settings; the access
            // token itself lives in the OS keychain, never on this path.
            match handle.path().app_config_dir() {
                Ok(config_dir) => {
                    state
                        .backend
                        .github()
                        .initialize(config_dir.join("github-account.json"));
                }
                Err(error) => {
                    log::warn!(
                        "app config dir unavailable; github account stays in memory: {error}"
                    );
                }
            }

            // Periodically revalidate stale owner/org avatars in the
            // background for as long as the process lives. Setup runs on the
            // main thread, outside any Tokio runtime. Spawn through Tauri's
            // runtime, never tokio::spawn directly here.
            let icon_service = Arc::clone(&state.icons);
            tauri::async_runtime::spawn(async move {
                icon_service.run_periodic_refresh().await;
            });

            // One-shot cleanup of disk caches (remote icons, repo info)
            // untouched for 30+ days. Blocking IO, so it runs off the main
            // thread and never delays first paint; failures only log.
            let cleanup_icons = Arc::clone(&state.icons);
            let cleanup_backend = Arc::clone(&state.backend);
            tauri::async_runtime::spawn_blocking(move || {
                let icons_removed =
                    cleanup_icons.prune_old_entries(git_backend::icons::IconConfig::default().prune_after);
                let info_removed = cleanup_backend
                    .prune_remote_info_cache(git_backend::REMOTE_INFO_PRUNE_AFTER);
                if icons_removed > 0 || info_removed > 0 {
                    log::info!(
                        "cache cleanup removed {icons_removed} icons and {info_removed} repo-info entries"
                    );
                }
            });

            // Persisted syntax themes must be active before the warm-up
            // snapshot is taken, otherwise the first diffs use defaults.
            commands::settings::apply_syntax_theme(&handle.state::<AppState>());

            // Warm the syntax highlighter (grammar dumps + embedded themes)
            // off the main thread so the first diff paints fully styled.
            tauri::async_runtime::spawn_blocking(git_backend::engines::gix::highlight::warm_up);

            let mut rx = handle.state::<AppState>().backend.subscribe_invalidations();
            tauri::async_runtime::spawn(async move {
                use tauri::Emitter;
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let _ = handle.emit("repository-invalidated", &event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            log::info!("setup complete in {:?}", started_at.elapsed());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Flush the session document whenever the window goes away:
            // frontend saves are debounced, so the tail of recent changes
            // may exist only in the backend's in-memory copy. Idempotent,
            // so reacting to both events is safe.
            if matches!(
                event,
                tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
            ) {
                let state = window.app_handle().state::<AppState>();
                if let Err(error) = state.session.persist_current() {
                    log::error!("could not flush session on close: {error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::repository::git_open_repository,
            commands::repository::git_close_repository,
            commands::repository::git_remove_repository,
            commands::repository::git_repository_snapshot,
            commands::repository::git_init_repository,
            commands::repository::git_list_gitignore_templates,
            commands::repository::git_list_licenses,
            commands::repository::git_stash_list,
            commands::repository::git_open_diff,
            commands::queries::git_history_page,
            commands::queries::git_history_chart,
            commands::queries::git_open_graph,
            commands::queries::git_read_graph_range,
            commands::queries::git_commit_detail,
            commands::queries::git_file_at_revision,
            commands::queries::git_blame,
            commands::queries::git_list_branches_and_tags,
            commands::queries::git_read_diff_image,
            commands::queries::git_read_diff_range,
            commands::queries::git_cancel_operation,
            commands::changes::git_status,
            commands::changes::git_stage_paths,
            commands::changes::git_unstage_paths,
            commands::changes::git_discard_changes,
            commands::changes::git_commit,
            commands::changes::git_amend_commit,
            commands::hooks::git_list_commit_hooks,
            commands::hooks::git_run_commit_hook,
            commands::hooks::git_read_commit_hook,
            commands::hooks::git_write_commit_hook,
            commands::mutations::git_create_branch,
            commands::mutations::git_delete_branch,
            commands::mutations::git_rename_branch,
            commands::mutations::git_create_tag,
            commands::mutations::git_delete_tag,
            commands::mutations::git_checkout,
            commands::mutations::git_reset,
            commands::workflows::git_merge,
            commands::workflows::git_merge_continue,
            commands::workflows::git_merge_abort,
            commands::workflows::git_operation_state,
            commands::workflows::git_resolve_conflict,
            commands::workflows::git_conflict_file,
            commands::workflows::git_start_rebase,
            commands::workflows::git_continue_rebase,
            commands::workflows::git_abort_rebase,
            commands::workflows::git_cherry_pick,
            commands::workflows::git_revert,
            commands::workflows::git_stash_push,
            commands::workflows::git_stash_pop,
            commands::remote_info::git_remote_repo_info,
            commands::remote_info::git_remote_repo_info_by_path,
            commands::remotes::git_list_remotes,
            commands::remotes::git_list_remotes_by_path,
            commands::remotes::git_add_remote,
            commands::remotes::git_remove_remote,
            commands::remotes::git_set_remote_url,
            commands::remotes::git_fetch,
            commands::remotes::git_push,
            commands::remotes::git_pull,
            commands::remotes::git_clone,
            commands::github::github_account,
            commands::github::github_begin_sign_in,
            commands::github::github_complete_sign_in,
            commands::github::github_cancel_sign_in,
            commands::github::github_sign_out,
            commands::github::github_list_orgs,
            commands::github::github_publish_repository,
            commands::github::github_list_notifications,
            commands::github::github_mark_notification_read,
            commands::github::github_mark_all_notifications_read,
            commands::github::github_resolve_subject_url,
            commands::github::github_list_issues,
            commands::github::github_get_issue,
            commands::github::github_list_issue_comments,
            commands::github::github_list_issue_events,
            commands::github::github_create_issue_comment,
            commands::github::github_update_issue,
            commands::github::github_create_issue,
            commands::github::github_update_issue_comment,
            commands::github::github_delete_issue_comment,
            commands::github::github_repo_permissions,
            commands::settings::settings_load,
            commands::settings::settings_set,
            commands::editor::open_in_editor,
            commands::file_manager::reveal_in_file_manager,
            commands::submodules::git_list_submodules,
            commands::submodules::git_add_submodule,
            commands::submodules::git_update_submodules,
            commands::worktrees::git_list_worktrees,
            commands::worktrees::git_create_worktree,
            commands::worktrees::git_remove_worktree,
            commands::worktrees::git_lock_worktree,
            commands::worktrees::git_unlock_worktree,
            commands::lfs::git_lfs_status,
            commands::lfs::git_lfs_smudge,
            commands::icons::icon_resolve,
            commands::icons::icon_refresh,
            commands::icons::git_repo_icon,
            commands::icons::git_repo_icon_by_path,
            commands::session::session_load,
            commands::session::session_save,
        ])
        .on_page_load(move |webview, payload| {
            if webview.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                log::info!(
                    "main webview finished loading in {:?}",
                    started_at.elapsed()
                );
                // The frontend reveals the window once the theme is applied
                // and the first frame has committed, so the pre-render page
                // never becomes visible. This is the fallback for a frontend
                // that never gets that far (bundle failure, crash before
                // render): without it the window would stay hidden with
                // nothing on screen to explain why.
                let window = webview.window();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if !matches!(window.is_visible(), Ok(true)) {
                        let _ = window.show();
                    }
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn external_navigation_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_opener::OpenerExt;

    tauri::plugin::Builder::<R>::new("external-navigation")
        .on_navigation(|webview, url| {
            let is_internal_host = matches!(
                url.host_str(),
                Some("localhost") | Some("127.0.0.1") | Some("tauri.localhost") | Some("::1")
            );

            let is_internal = url.scheme() == "tauri" || is_internal_host;

            if is_internal {
                return true;
            }

            let is_external_link = matches!(url.scheme(), "http" | "https" | "mailto" | "tel");

            if is_external_link {
                log::info!("opening external link in system browser: {}", url);
                let _ = webview.opener().open_url(url.as_str(), None::<&str>);
                return false;
            }

            true
        })
        .build()
}
