//! End-to-end publish orchestration: fake provider, local bare remote.
//! Exercises create -> origin setup -> push with upstream, offline.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use forge_core::dto::{AccountProfile, CreateRepositoryBody, CreatedRepository};
use forge_core::{
    AuthChallenge, AuthPoll, Capabilities, ForgeAccounts, ForgeFuture, ForgeProvider,
    ForgeRegistry, MemoryTokenStore,
};
use git_backend::Backend;

mod common;
use common::TestRepo;

struct FakeProvider {
    created: Mutex<Option<CreatedRepository>>,
    bare_remote: PathBuf,
}

impl FakeProvider {
    fn new(bare_remote: PathBuf) -> Self {
        Self {
            created: Mutex::new(None),
            bare_remote,
        }
    }
}

impl ForgeProvider for FakeProvider {
    fn id(&self) -> &'static str {
        "github"
    }

    fn label(&self) -> &'static str {
        "GitHub"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            publish: true,
            ..Default::default()
        }
    }

    fn hosts(&self) -> &'static [&'static str] {
        &["github.com"]
    }

    fn parse_remote(&self, _url: &str) -> Option<(String, String, String)> {
        None
    }

    fn request_device_code(
        &self,
        _client_id: &str,
        _scopes: &str,
    ) -> ForgeFuture<(AuthChallenge, String)> {
        Box::pin(async {
            Ok((
                AuthChallenge {
                    user_code: "ABCD-1234".into(),
                    verification_uri: "https://github.com/login/device".into(),
                    expires_in_secs: 900,
                },
                "dev-1".into(),
            ))
        })
    }

    fn poll_token(&self, _client_id: &str, _device_code: &str) -> ForgeFuture<AuthPoll> {
        Box::pin(async {
            Ok(AuthPoll::Authorized {
                token: "tok-123".into(),
                scopes: vec!["repo".into(), "read:user".into(), "workflow".into()],
            })
        })
    }

    fn authenticated_user(&self, _token: &str) -> ForgeFuture<AccountProfile> {
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

    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepositoryBody,
    ) -> ForgeFuture<CreatedRepository> {
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
}

fn backend_with_fake(bare_remote: &Path) -> (Backend, Arc<FakeProvider>) {
    let provider = Arc::new(FakeProvider::new(bare_remote.to_path_buf()));
    let registry = Arc::new(ForgeRegistry::new(vec![provider.clone()]));
    let mut accounts = ForgeAccounts::new(registry, Box::new(MemoryTokenStore::default()));
    accounts.set_client_id("github", "test-client-id");
    let backend = Backend::with_accounts(Default::default(), Arc::new(accounts));
    (backend, provider)
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
        backend
            .forge_begin_sign_in("github".into(), "github.com".into())
            .await
            .unwrap();
        let connection = backend.forge_complete_sign_in().await.unwrap();
        assert_eq!(connection.account.login, "octocat");
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
