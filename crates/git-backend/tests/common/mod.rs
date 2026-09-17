#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub struct TestRepo {
    pub root: PathBuf,
    pub repo: git2::Repository,
}

impl TestRepo {
    pub fn init(name: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(name);
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        config.set_str("core.autocrlf", "false").unwrap();
        drop(config);
        std::mem::forget(dir);
        Self { root, repo }
    }

    pub fn write(&self, rel: &str, content: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap()
    }

    pub fn delete(&self, rel: &str) {
        std::fs::remove_file(self.root.join(rel)).unwrap();
    }

    pub fn stage(&self, rel: &str) {
        let mut index = self.repo.index().unwrap();
        let path = Path::new(rel);
        let _ = index.conflict_remove(path);
        index.add_path(path).unwrap_or_else(|e| {
            panic!("add_path {rel} failed: {e}");
        });
        index.write().unwrap();
    }

    pub fn commit_all(&self, message: &str) -> git2::Oid {
        let mut index = self.repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
        let parents: Vec<git2::Commit> = match self.repo.head() {
            Ok(head) => vec![head.peel_to_commit().unwrap()],
            Err(_) => vec![],
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }

    pub fn initial_commit(&self, files: &[(&str, &str)]) -> git2::Oid {
        for (rel, content) in files {
            self.write(rel, content);
        }
        self.commit_all("initial")
    }

    pub fn head_commit(&self) -> git2::Commit<'_> {
        self.repo.head().unwrap().peel_to_commit().unwrap()
    }
}
