use git_backend::api::repository::CreateRepositoryRequest;
use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn request(parent: &std::path::Path, name: &str, readme: bool) -> CreateRepositoryRequest {
    CreateRepositoryRequest {
        parent_directory: parent.to_string_lossy().into_owned(),
        name: name.into(),
        readme,
        gitignore_template: None,
        license: None,
    }
}

#[test]
fn creates_repository_with_scaffolding_files() {
    let outer = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());

    let result = futures_block(backend.create_repository(CreateRepositoryRequest {
        parent_directory: outer.path().to_string_lossy().into_owned(),
        name: "scaffolded".into(),
        readme: true,
        gitignore_template: Some("rust".into()),
        license: Some("mit".into()),
    }))
    .unwrap();

    let root = std::path::PathBuf::from(&result.path);
    assert!(root.join(".git").exists(), "git init must run");
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert_eq!(readme, "# scaffolded\n");
    let gitignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(gitignore.contains("/target/"));
    let license = std::fs::read_to_string(root.join("LICENSE")).unwrap();
    assert!(license.starts_with("Copyright (c) "));
    assert!(license.contains("Permission is hereby granted"));

    // The returned repo is immediately usable (snapshot came from the open).
    assert!(!result.repo.snapshot.git_dir.as_os_str().is_empty());
}

#[test]
fn empty_request_writes_nothing_extra() {
    let outer = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());
    let result =
        futures_block(backend.create_repository(request(outer.path(), "bare-bones", false)))
            .unwrap();

    let root = std::path::PathBuf::from(&result.path);
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, vec![".git"]);
}

#[test]
fn rejects_missing_parent_paths_that_cannot_hold_a_repo() {
    let outer = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());

    let missing_parent =
        futures_block(backend.create_repository(request(&outer.path().join("nope"), "repo", true)))
            .unwrap_err();
    assert_eq!(missing_parent.code(), "invalidInput");

    let file = outer.path().join("file.txt");
    std::fs::write(&file, "data").unwrap();
    let not_a_directory =
        futures_block(backend.create_repository(request(&file, "repo", true))).unwrap_err();
    assert_eq!(not_a_directory.code(), "invalidInput");

    let invalid_name =
        futures_block(backend.create_repository(request(outer.path(), "..", true))).unwrap_err();
    assert_eq!(invalid_name.code(), "invalidInput");
}

#[test]
fn initializes_an_existing_folder_with_its_files_kept() {
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("existing.txt"), "data").unwrap();
    std::fs::create_dir(outer.path().join("src")).unwrap();
    let backend = Backend::new(BackendConfig::default());

    let result = futures_block(backend.create_repository(request(outer.path(), "", true))).unwrap();

    let root = std::path::PathBuf::from(&result.path);
    assert_eq!(root, outer.path().canonicalize().unwrap());
    assert!(root.join(".git").exists(), "git init must run");
    assert!(root.join("existing.txt").exists(), "files are kept");
    assert!(root.join("src").exists(), "folders are kept");
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.starts_with("# "), "title comes from the folder name");
}

#[test]
fn initializes_a_named_folder_that_already_has_files() {
    let outer = tempfile::tempdir().unwrap();
    let target = outer.path().join("project");
    std::fs::create_dir_all(target.join("src")).unwrap();
    std::fs::write(target.join("main.rs"), "fn main() {}").unwrap();
    let backend = Backend::new(BackendConfig::default());

    let result =
        futures_block(backend.create_repository(request(outer.path(), "project", true))).unwrap();

    let root = std::path::PathBuf::from(&result.path);
    assert_eq!(root, target.canonicalize().unwrap());
    assert!(root.join(".git").exists(), "git init must run");
    assert!(root.join("main.rs").exists(), "files are kept");
}

#[test]
fn scaffolding_never_overwrites_existing_files() {
    let outer = tempfile::tempdir().unwrap();
    let target = outer.path().join("project");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("README.md"), "mine").unwrap();
    std::fs::write(target.join("LICENSE"), "mine").unwrap();
    let backend = Backend::new(BackendConfig::default());

    futures_block(backend.create_repository(CreateRepositoryRequest {
        parent_directory: outer.path().to_string_lossy().into_owned(),
        name: "project".into(),
        readme: true,
        gitignore_template: Some("rust".into()),
        license: Some("mit".into()),
    }))
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(target.join("README.md")).unwrap(),
        "mine"
    );
    assert_eq!(
        std::fs::read_to_string(target.join("LICENSE")).unwrap(),
        "mine"
    );
    // Absent files are still written.
    assert!(
        std::fs::read_to_string(target.join(".gitignore"))
            .unwrap()
            .contains("/target/")
    );
}

#[test]
fn rejects_folders_that_are_already_repositories() {
    let outer = tempfile::tempdir().unwrap();
    let repo = outer.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    drop(git2::Repository::init(&repo).unwrap());
    let backend = Backend::new(BackendConfig::default());

    // The folder itself is a repository.
    let error = futures_block(backend.create_repository(request(&repo, "", false))).unwrap_err();
    assert_eq!(error.code(), "conflict");

    // The folder that would be created already exists as a repository.
    let error =
        futures_block(backend.create_repository(request(outer.path(), "repo", false))).unwrap_err();
    assert_eq!(error.code(), "conflict");
}

#[test]
fn allows_a_repository_nested_inside_another_one() {
    let outer = tempfile::tempdir().unwrap();
    drop(git2::Repository::init(outer.path()).unwrap());
    let backend = Backend::new(BackendConfig::default());

    let result =
        futures_block(backend.create_repository(request(outer.path(), "nested", false))).unwrap();

    assert!(std::path::Path::new(&result.path).join(".git").exists());
}
