use git_backend::api::repository::CreateRepositoryRequest;
use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

fn request(parent: &std::path::Path, name: &str, readme: bool) -> CreateRepositoryRequest {
    CreateRepositoryRequest {
        parent_directory: parent.to_string_lossy().into_owned(),
        name: name.into(),
        init_in_place: false,
        readme,
        gitignore_template: None,
        license: None,
    }
}

fn in_place_request(parent: &std::path::Path, readme: bool) -> CreateRepositoryRequest {
    CreateRepositoryRequest {
        parent_directory: parent.to_string_lossy().into_owned(),
        name: String::new(),
        init_in_place: true,
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
        init_in_place: false,
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
fn rejects_missing_parent_and_nonempty_targets() {
    let outer = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());

    let missing_parent =
        futures_block(backend.create_repository(request(&outer.path().join("nope"), "repo", true)))
            .unwrap_err();
    assert_eq!(missing_parent.code(), "invalidInput");

    let occupied = outer.path().join("occupied");
    std::fs::create_dir_all(&occupied).unwrap();
    std::fs::write(occupied.join("existing.txt"), "data").unwrap();
    let conflict =
        futures_block(backend.create_repository(request(outer.path(), "occupied", true)))
            .unwrap_err();
    assert_eq!(conflict.code(), "conflict");

    let invalid_name =
        futures_block(backend.create_repository(request(outer.path(), "..", true))).unwrap_err();
    assert_eq!(invalid_name.code(), "invalidInput");
}

#[test]
fn initializes_in_place_inside_existing_directory_with_files() {
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join("existing.txt"), "data").unwrap();
    std::fs::create_dir(outer.path().join("src")).unwrap();
    let backend = Backend::new(BackendConfig::default());

    let result =
        futures_block(backend.create_repository(in_place_request(outer.path(), true))).unwrap();

    let root = std::path::PathBuf::from(&result.path);
    assert_eq!(root, outer.path().canonicalize().unwrap());
    assert!(root.join(".git").exists(), "git init must run");
    assert!(root.join("existing.txt").exists(), "files are kept");
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(readme.starts_with("# "), "title comes from the folder name");
}

#[test]
fn rejects_in_place_init_inside_an_existing_repository() {
    let outer = tempfile::tempdir().unwrap();
    let repo = git2::Repository::init(outer.path()).unwrap();
    drop(repo);
    let backend = Backend::new(BackendConfig::default());

    let error = futures_block(backend.create_repository(in_place_request(outer.path(), false)))
        .unwrap_err();
    assert_eq!(error.code(), "conflict");

    // Existing non-repo folders stay allowed.
    let plain = tempfile::tempdir().unwrap();
    std::fs::write(plain.path().join("note.txt"), "keep me").unwrap();
    let ok =
        futures_block(backend.create_repository(in_place_request(plain.path(), false))).unwrap();
    assert!(std::path::Path::new(&ok.path).join(".git").exists());
}
