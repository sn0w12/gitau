use crate::domain::history::FileContent;
use crate::domain::{ObjectId, RelativePath};
use crate::engines::gix::session::GixSession;
use crate::error::{GitError, Result};

pub fn blob_at(
    session: &GixSession,
    revision: ObjectId,
    path: &RelativePath,
) -> Result<FileContent> {
    let repo = session.handle();
    let gix_revision = gix::ObjectId::try_from(revision)?;
    let commit = repo
        .find_commit(gix_revision)
        .map_err(|_| GitError::ObjectNotFound {
            oid: revision.hex(),
        })?;
    let tree_id = commit.tree_id().map_err(|e| GitError::Internal {
        message: e.to_string(),
    })?;

    let mut current_tree_id = tree_id.detach();
    let components: Vec<&str> = path.as_str().split('/').collect();
    for (index, component) in components.iter().enumerate() {
        let is_last = index == components.len() - 1;
        let obj = repo
            .find_object(current_tree_id)
            .map_err(|_| GitError::ObjectNotFound {
                oid: current_tree_id.to_string(),
            })?;
        if obj.kind != gix::objs::Kind::Tree {
            return Err(GitError::ObjectNotFound {
                oid: format!("{revision}:{path}"),
            });
        }
        let tree = gix::objs::TreeRef::from_bytes(&obj.data, repo.object_hash()).map_err(|e| {
            GitError::Internal {
                message: e.to_string(),
            }
        })?;

        let wanted = gix::bstr::BStr::new(component.as_bytes());
        let entry = tree
            .entries
            .iter()
            .find(|entry| entry.filename == wanted)
            .ok_or_else(|| GitError::ObjectNotFound {
                oid: format!("{revision}:{path}"),
            })?;

        let kind = entry.mode.kind();
        if is_last {
            if kind != gix::objs::tree::EntryKind::Blob {
                return Err(GitError::ObjectNotFound {
                    oid: format!("{revision}:{path}"),
                });
            }
            let blob_oid = gix::ObjectId::from(entry.oid);
            let blob = repo
                .find_blob(blob_oid)
                .map_err(|_| GitError::ObjectNotFound {
                    oid: blob_oid.to_string(),
                })?;
            let data = &blob.data;
            let binary = data[..data.len().min(8000)].contains(&0);
            return Ok(FileContent {
                path: path.clone(),
                revision,
                data: data.clone(),
                binary,
                size: data.len() as u64,
            });
        }

        if kind != gix::objs::tree::EntryKind::Tree {
            return Err(GitError::ObjectNotFound {
                oid: format!("{revision}:{path}"),
            });
        }
        current_tree_id = gix::ObjectId::from(entry.oid);
    }
    Err(GitError::ObjectNotFound {
        oid: format!("{revision}:{path}"),
    })
}
