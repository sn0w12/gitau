use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

#[test]
fn opens_repository_by_mixed_separator_path() {
    // The dialogs compose destinations as `parent + "/" + name`; on Windows
    // the parent carries backslashes, so the string mixes separators. The
    // backend must open it exactly like a dialog-picked pure-backslash path.
    let outer = tempfile::tempdir().unwrap();
    let parent = outer.path();
    let target = parent.join("seeded");
    std::fs::create_dir_all(&target).unwrap();
    git2::Repository::init(&target).unwrap();

    let backend = Backend::new(BackendConfig::default());
    let parent_str = parent.to_string_lossy().into_owned();
    let mixed = format!("{parent_str}/seeded");
    assert!(
        mixed.contains('/'),
        "test input must mix separators: {mixed}"
    );

    let opened = futures_block(backend.open_repository(std::path::Path::new(&mixed)));
    match opened {
        Ok(_) => {}
        Err(error) => panic!("mixed-separator open failed: {error:?}"),
    }
}
