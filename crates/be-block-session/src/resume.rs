use be_block_commit::CommitId;
#[cfg(feature = "store")]
use be_block_commit::{CommitStore, Descent, MergeBase};
#[cfg(feature = "store")]
use be_block_store::{ObjectStore, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
    /// A seal of the session's order through `through`, made on the local head: the host tells
    /// this from a fast-forward by the commit's `sealed_through`.
    Sealed {
        commit: CommitId,
        through: u64,
        restarts: u64,
    },
}

/// How `local`, the head a replica is on, relates to `remote`, the server's. `left` is the
/// newest commit in `local`'s history the server took (`Peer::left_head`): `local` itself once it
/// was published, else the head it was built on. Past what the server thinned away of the
/// remote head's history (one commit a minute in a block's first hour), the remote head was
/// made on it, so a merge is based there, as a merge base walk falls back; with none, on
/// nothing.
#[cfg(feature = "store")]
pub fn resume<S: ObjectStore>(
    commits: &CommitStore<S>,
    local: Option<CommitId>,
    remote: Option<CommitId>,
    left: Option<CommitId>,
) -> Result<Resume, StoreError> {
    match (local, remote) {
        (None, None) => Ok(Resume::Empty),
        (None, Some(remote)) => Ok(Resume::FastForward { to: remote }),
        (Some(local), None) => Ok(Resume::Publish { from: local }),
        (Some(local), Some(remote)) if local == remote => Ok(Resume::UpToDate),
        (Some(local), Some(remote)) => {
            if commits.is_ancestor(local, remote)? == Descent::Yes {
                return Ok(Resume::FastForward { to: remote });
            }
            if commits.is_ancestor(remote, local)? == Descent::Yes {
                return Ok(Resume::Publish { from: local });
            }
            let base = match commits.common_ancestor(local, remote)? {
                MergeBase::Behind => return Ok(Resume::Publish { from: local }),
                MergeBase::Base(base) => Some(base),
                MergeBase::Unreached => left,
                MergeBase::Unrelated => None,
            };
            Ok(Resume::Merge {
                base,
                ours: local,
                theirs: remote,
            })
        }
    }
}
