//! Worktree icon discovery: filename matching, ignore rules, and depth
//! bounds against a real repository.

mod common;

use common::TestRepo;
use git_backend::engines::gix::worktree_icon::find_worktree_icon;

const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
const ICO: &[u8] = &[0x00, 0x00, 0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\"/>";

fn write_bytes(root: &std::path::Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, bytes).unwrap();
}

fn icon(repo: &TestRepo) -> git_backend::engines::gix::worktree_icon::WorktreeIcon {
    find_worktree_icon(&repo.root)
        .unwrap()
        .expect("expected an icon to be found")
}

#[test]
fn finds_tracked_icon_at_root() {
    let repo = TestRepo::init("icon-root");
    repo.write("favicon.png", "placeholder");
    write_bytes(&repo.root, "favicon.png", PNG);
    repo.commit_all("initial");

    let found = icon(&repo);
    assert_eq!(found.content_type, "image/png");
    assert!(found.modified_ms > 0);
}

#[test]
fn finds_untracked_icon() {
    let repo = TestRepo::init("icon-untracked");
    repo.initial_commit(&[("README.md", "hi")]);
    write_bytes(&repo.root, "icon.png", PNG);

    let found = icon(&repo);
    assert_eq!(found.content_type, "image/png");
}

#[test]
fn svg_icons_are_supported() {
    let repo = TestRepo::init("icon-svg");
    repo.initial_commit(&[("logo.svg", SVG)]);

    let found = icon(&repo);
    assert_eq!(found.content_type, "image/svg+xml");
}

#[test]
fn gitignored_paths_are_skipped_even_with_higher_priority_names() {
    let repo = TestRepo::init("icon-ignored");
    repo.initial_commit(&[
        (".gitignore", "vendor/\nblob\n"),
        ("logo.png", "placeholder"),
    ]);
    write_bytes(&repo.root, "logo.png", PNG);
    repo.commit_all("add logo");
    write_bytes(&repo.root, "vendor/favicon.ico", ICO);
    write_bytes(&repo.root, "blob/icon.png", PNG);

    let found = icon(&repo);
    assert_eq!(found.content_type, "image/png");
    assert_eq!(found.bytes, PNG);
}

#[test]
fn gitignored_untracked_files_are_skipped() {
    let repo = TestRepo::init("icon-ignored-file");
    repo.initial_commit(&[(".gitignore", "ignored-icon.png\n")]);
    write_bytes(&repo.root, "ignored-icon.png", PNG);
    repo.write("readme.txt", "nothing here");

    assert!(find_worktree_icon(&repo.root).unwrap().is_none());
}

#[test]
fn depth_beats_stem_priority() {
    let repo = TestRepo::init("icon-depth");
    repo.write("logo.png", "placeholder");
    repo.write("docs/brand/favicon.ico", "placeholder");
    write_bytes(&repo.root, "logo.png", PNG);
    write_bytes(&repo.root, "docs/brand/favicon.ico", ICO);
    repo.commit_all("initial");

    let found = icon(&repo);
    assert_eq!(found.content_type, "image/png");
}

#[test]
fn stem_beats_extension_priority() {
    let repo = TestRepo::init("icon-stem");
    repo.write("icon.svg", SVG);
    repo.write("logo.png", "placeholder");
    write_bytes(&repo.root, "logo.png", PNG);
    repo.commit_all("initial");

    assert_eq!(icon(&repo).content_type, "image/svg+xml");
}

#[test]
fn icons_beyond_max_depth_are_not_found() {
    let repo = TestRepo::init("icon-deep");
    repo.write("a/b/c/d/icon.png", "placeholder");
    repo.commit_all("initial");

    assert!(find_worktree_icon(&repo.root).unwrap().is_none());
}

#[test]
fn icons_at_max_depth_are_found() {
    let repo = TestRepo::init("icon-max-depth");
    repo.write("a/b/c/icon.png", "placeholder");
    write_bytes(&repo.root, "a/b/c/icon.png", PNG);
    repo.commit_all("initial");

    assert!(!icon(&repo).bytes.is_empty());
}

#[test]
fn worktrees_without_icons_resolve_to_none() {
    let repo = TestRepo::init("icon-none");
    repo.initial_commit(&[("src/main.rs", "fn main() {}\n")]);

    assert!(find_worktree_icon(&repo.root).unwrap().is_none());
}

#[test]
fn non_image_files_matching_icon_names_are_rejected() {
    let repo = TestRepo::init("icon-not-image");
    repo.initial_commit(&[("favicon.ico", "<html>definitely not an image</html>")]);

    assert!(find_worktree_icon(&repo.root).unwrap().is_none());
}
