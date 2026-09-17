#![allow(dead_code)]

use std::time::Instant;

fn median(xs: &mut [f64]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

#[test]
#[ignore = "manual perf probe"]
fn status_phase_probe() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("probe-status");
    std::fs::create_dir_all(&root).unwrap();
    let repo = git2::Repository::init(&root).unwrap();
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "P").unwrap();
    config.set_str("user.email", "p@e.c").unwrap();
    config.set_str("core.autocrlf", "false").unwrap();
    drop(config);

    for i in 0..200u32 {
        let content: String = (0..50).map(|j| format!("file {i} line {j}\n")).collect();
        std::fs::write(root.join(format!("file{i}.txt")), content).unwrap();
    }
    let mut index = repo.index().unwrap();
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let sig = git2::Signature::now("P", "p@e.c").unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
        .unwrap();

    let changed: String = (0..50).map(|j| format!("CHANGED line {j}\n")).collect();
    for i in 0..100u32 {
        std::fs::write(root.join(format!("file{i}.txt")), &changed).unwrap();
    }

    // Warm handle like production TLS reuse.
    let full_opts = || {
        let mut o = git2::StatusOptions::new();
        o.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false)
            .include_unmodified(false)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .sort_case_insensitively(false)
            .update_index(true);
        o
    };

    let mut t_status_full = Vec::new();
    let mut t_status_bare = Vec::new();
    let mut t_diff_wt = Vec::new();
    let mut t_diff_head_idx = Vec::new();
    for _ in 0..30 {
        let now = Instant::now();
        let s = repo.statuses(Some(&mut full_opts())).unwrap();
        assert_eq!(s.len(), 100);
        t_status_full.push(now.elapsed().as_secs_f64() * 1e3);

        let now = Instant::now();
        let s = repo.statuses(None).unwrap();
        assert_eq!(s.len(), 100);
        t_status_bare.push(now.elapsed().as_secs_f64() * 1e3);

        let now = Instant::now();
        let d = repo.diff_index_to_workdir(None, None).unwrap();
        assert_eq!(d.deltas().count(), 100);
        t_diff_wt.push(now.elapsed().as_secs_f64() * 1e3);

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let now = Instant::now();
        let d = repo
            .diff_tree_to_index(
                Some(&head.tree().unwrap()),
                Some(&repo.index().unwrap()),
                None,
            )
            .unwrap();
        assert_eq!(d.deltas().count(), 0);
        t_diff_head_idx.push(now.elapsed().as_secs_f64() * 1e3);
    }

    println!(
        "status-full      median {:.3} ms",
        median(&mut t_status_full)
    );
    println!(
        "status-bare      median {:.3} ms",
        median(&mut t_status_bare)
    );
    println!("diff-index-wt    median {:.3} ms", median(&mut t_diff_wt));
    println!(
        "diff-tree-index  median {:.3} ms",
        median(&mut t_diff_head_idx)
    );

    let grepo = gix::discover(&root).unwrap();
    let mut t_gix = Vec::new();
    for _ in 0..30 {
        let now = Instant::now();
        let iter = grepo
            .status(gix::progress::Discard)
            .unwrap()
            .untracked_files(gix::status::UntrackedFiles::Files)
            .into_iter(None)
            .unwrap();
        let n = iter.filter_map(Result::ok).count();
        t_gix.push(now.elapsed().as_secs_f64() * 1e3);
        assert!(n >= 100);
    }
    println!("gix-status       median {:.3} ms", median(&mut t_gix));
}
