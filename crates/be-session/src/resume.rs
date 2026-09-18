use be_commit::{CommitId, CommitStore};
use be_store::{ObjectStore, StoreError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Resume {
    Empty,
    UpToDate,
    FastForward {
        to: CommitId,
    },
    Publish {
        from: CommitId,
    },
    Merge {
        base: Option<CommitId>,
        ours: CommitId,
        theirs: CommitId,
    },
}

pub fn resume<S: ObjectStore>(
    commits: &CommitStore<S>,
    local: Option<CommitId>,
    remote: Option<CommitId>,
) -> Result<Resume, StoreError> {
    match (local, remote) {
        (None, None) => Ok(Resume::Empty),
        (None, Some(remote)) => Ok(Resume::FastForward { to: remote }),
        (Some(local), None) => Ok(Resume::Publish { from: local }),
        (Some(local), Some(remote)) if local == remote => Ok(Resume::UpToDate),
        (Some(local), Some(remote)) => {
            if commits.is_ancestor(local, remote)? {
                return Ok(Resume::FastForward { to: remote });
            }
            if commits.is_ancestor(remote, local)? {
                return Ok(Resume::Publish { from: local });
            }
            Ok(Resume::Merge {
                base: commits.common_ancestor(local, remote)?,
                ours: local,
                theirs: remote,
            })
        }
    }
}

pub fn takeover_needs_merge(clean_at: Option<CommitId>, head: Option<CommitId>) -> bool {
    match (clean_at, head) {
        (Some(clean), Some(head)) => clean != head,
        (None, None) => false,
        _ => true,
    }
}
