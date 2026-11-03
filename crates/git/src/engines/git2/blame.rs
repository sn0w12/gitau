use git2::BlameOptions;

use crate::api::history::BlameQuery;
use crate::domain::history::{BlameLine, BlameResult};
use crate::domain::{ObjectId, RelativePath};
use crate::error::{GitError, Result};

pub fn blame(repo: &git2::Repository, query: &BlameQuery) -> Result<BlameResult> {
    let path = RelativePath::parse(&query.path)?;
    let mut options = BlameOptions::default();
    options.ignore_whitespace(query.ignore_whitespace);
    if let Some(revision) = &query.revision {
        let commit = crate::streaming::pipeline::resolve_commit(repo, revision)?;
        options.newest_commit(commit.id());
    }

    let path_buf = path.to_path_buf();
    let blame = repo
        .blame_file(&path_buf, Some(&mut options))
        .map_err(|e| GitError::ObjectNotFound {
            oid: format!("{}: {}", path, e.message()),
        })?;

    let mut lines = Vec::new();
    for hunk in blame.iter() {
        let commit = ObjectId::from_bytes(hunk.final_commit_id().as_bytes())?;
        let boundary = hunk.is_boundary();
        let start = hunk.final_start_line();
        for offset in 0..hunk.lines_in_hunk() {
            lines.push(BlameLine {
                line_no: (start + offset) as u32,
                commit,
                boundary,
            });
        }
    }
    lines.sort_by_key(|line| line.line_no);

    Ok(BlameResult {
        path,
        revision: None,
        lines,
    })
}
