//! End-to-end publish orchestration: fake GitHub API, local bare remote.
//! Exercises create -> origin setup -> push with upstream, offline.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use git_backend::Backend;
use git_backend::api::github::{AccountProfile, DeviceFlowStart, GithubOrg, NotificationPage};
use git_backend::github::api::{GithubApi, GithubFuture};
use git_backend::github::device_flow::{DeviceCodeResponse, TokenPoll};
use git_backend::github::{CreateRepoBody, CreatedRepository, GitHubAuth, MemoryTokenStore};

mod common;
use common::TestRepo;

struct FakeApi {
    created: Mutex<Option<CreatedRepository>>,
    bare_remote: PathBuf,
}

impl FakeApi {
    fn new(bare_remote: PathBuf) -> Self {
        Self {
            created: Mutex::new(None),
            bare_remote,
        }
    }
}

fn start_response() -> DeviceFlowStart {
    DeviceFlowStart {
        user_code: "ABCD-1234".into(),
        verification_uri: "https://github.com/login/device".into(),
        expires_in_secs: 900,
    }
}

impl GithubApi for FakeApi {
    fn request_device_code(
        &self,
        _client_id: &str,
        _scopes: &str,
    ) -> GithubFuture<DeviceCodeResponse> {
        Box::pin(async {
            Ok(DeviceCodeResponse {
                device_code: "dev-1".into(),
                user_code: start_response().user_code,
                verification_uri: start_response().verification_uri,
                expires_in_secs: 900,
                interval_secs: 1,
            })
        })
    }

    fn poll_token(&self, _client_id: &str, _device_code: &str) -> GithubFuture<TokenPoll> {
        Box::pin(async {
            Ok(TokenPoll::Authorized {
                access_token: "tok-123".into(),
                scopes: vec!["repo".into(), "read:user".into(), "workflow".into()],
            })
        })
    }

    fn authenticated_user(&self, _token: &str) -> GithubFuture<AccountProfile> {
        Box::pin(async {
            Ok(AccountProfile {
                login: "octocat".into(),
                name: Some("The Octocat".into()),
                avatar_url: "https://avatars.githubusercontent.com/u/1".into(),
                html_url: "https://github.com/octocat".into(),
                scopes: vec![],
                connected_at_ms: 0,
            })
        })
    }

    fn list_orgs(&self, _token: &str) -> GithubFuture<Vec<GithubOrg>> {
        Box::pin(async {
            Ok(vec![GithubOrg {
                login: "acme".into(),
                avatar_url: None,
            }])
        })
    }

    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepoBody,
    ) -> GithubFuture<CreatedRepository> {
        assert_eq!(token, "tok-123");
        let full_name = format!("{}/{}", owner.unwrap_or("octocat"), body.name);
        let created = CreatedRepository {
            full_name,
            html_url: String::new(),
            default_branch: None,
            // Local stand-in for the HTTPS clone URL keeps the push offline.
            clone_url: Some(self.bare_remote.to_string_lossy().into_owned()),
        };
        *self.created.lock().unwrap() = Some(created.clone());
        Box::pin(async move { Ok(created) })
    }

    fn list_notifications(&self, _token: &str, page: u32) -> GithubFuture<NotificationPage> {
        Box::pin(async move {
            Ok(NotificationPage {
                notifications: vec![],
                page,
                has_more: false,
            })
        })
    }

    fn mark_notification_read(&self, _token: &str, _thread_id: &str) -> GithubFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn mark_all_notifications_read(&self, _token: &str) -> GithubFuture<()> {
        Box::pin(async { Ok(()) })
    }

    fn fetch_subject_html_url(
        &self,
        _token: &str,
        _subject_url: &str,
    ) -> GithubFuture<Option<String>> {
        Box::pin(async { Ok(None) })
    }
}

fn backend_with_fake(bare_remote: &Path) -> (Backend, Arc<FakeApi>) {
    let api = Arc::new(FakeApi::new(bare_remote.to_path_buf()));
    let auth = Arc::new(GitHubAuth::with_client_id(
        api.clone(),
        Box::new(MemoryTokenStore::default()),
        "test-client-id".into(),
    ));
    let backend = Backend::with_github(Default::default(), auth);
    (backend, api)
}

fn open(backend: &Backend, repo: &TestRepo) -> u64 {
    let opened = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(backend.open_repository(&repo.root))
        .unwrap();
    opened.id.0
}

#[test]
fn publish_creates_repo_wires_origin_and_pushes_upstream() {
    let repo = TestRepo::init("publish-me");
    repo.initial_commit(&[("README.md", "hello")]);

    let bare = tempfile::tempdir().unwrap();
    let bare_path = bare.path().join("remote.git");
    git2::Repository::init_bare(&bare_path).unwrap();

    let (backend, api) = backend_with_fake(&bare_path);
    let id = open(&backend, &repo);

    // Sign in first: publish refuses to run without a stored token.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        backend.github_begin_sign_in().await.unwrap();
        let profile = backend.github_complete_sign_in().await.unwrap();
        assert_eq!(profile.login, "octocat");
    });

    let result = rt
        .block_on(backend.publish_repository(
            git_backend::domain::RepoId(id),
            git_backend::PublishRepositoryRequest {
                name: "publish-me".into(),
                ..Default::default()
            },
            None,
        ))
        .unwrap();

    assert_eq!(result.full_name, "octocat/publish-me");
    let head_branch = repo.repo.head().unwrap().shorthand().unwrap().to_owned();
    assert_eq!(result.default_branch.as_deref(), Some(head_branch.as_str()));

    // The remote points at the URL the API reported and the branch landed
    // there with upstream tracking configured.
    let git_repo = git2::Repository::discover(&repo.root).unwrap();
    let url_starts_with_remote = {
        let remote = git_repo.find_remote("origin").unwrap();
        remote
            .url()
            .unwrap()
            .starts_with(bare_path.to_str().unwrap())
    };
    assert!(
        url_starts_with_remote,
        "origin should point at the created repository"
    );
    let branch = git_repo
        .find_branch(&head_branch, git2::BranchType::Local)
        .unwrap();
    assert!(branch.upstream().is_ok());
    drop(branch);
    drop(git_repo);

    let bare_repo = git2::Repository::open_bare(&bare_path).unwrap();
    assert!(
        bare_repo
            .revparse_single(&format!("refs/heads/{head_branch}"))
            .is_ok(),
        "the pushed branch should exist on the remote"
    );
    drop(bare_repo);

    let created = api.created.lock().unwrap().clone().unwrap();
    assert_eq!(created.full_name, "octocat/publish-me");
}

#[test]
fn publish_rejects_unborn_repository_before_touching_the_api() {
    let repo = TestRepo::init("empty");

    let bare = tempfile::tempdir().unwrap();
    let (backend, api) = backend_with_fake(bare.path());
    let id = open(&backend, &repo);

    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(backend.publish_repository(
            git_backend::domain::RepoId(id),
            git_backend::PublishRepositoryRequest {
                name: "some-repo".into(),
                ..Default::default()
            },
            None,
        ))
        .unwrap_err();

    assert_eq!(error.code(), "invalidInput");
    assert!(api.created.lock().unwrap().is_none());
}

#[test]
fn publish_rejects_empty_name_before_touching_the_api() {
    let repo = TestRepo::init("named");
    repo.initial_commit(&[("a.txt", "a")]);

    let bare = tempfile::tempdir().unwrap();
    let (backend, api) = backend_with_fake(bare.path());
    let id = open(&backend, &repo);

    let error = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(backend.publish_repository(
            git_backend::domain::RepoId(id),
            git_backend::PublishRepositoryRequest {
                name: "   ".into(),
                ..Default::default()
            },
            None,
        ))
        .unwrap_err();

    assert_eq!(error.code(), "invalidInput");
    assert!(api.created.lock().unwrap().is_none());
}
