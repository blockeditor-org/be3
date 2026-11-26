use be_block_commit::CommitId;
use uuid::Uuid;

use crate::seen::Edit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Ours,
    Theirs,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Comparison {
    pub block: Uuid,
    pub ours: Option<Result<Uuid, String>>,
    pub theirs: Option<Result<Uuid, String>>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conflicted {
    pub ours: CommitId,
    pub theirs: CommitId,
    pub count: usize,
}

pub fn conflicted(block: Uuid) -> Option<Conflicted> {
    super::with_seen(|shared| {
        let conflicts = shared.conflicts.get(&block)?;
        let last = conflicts.last()?;
        Some(Conflicted {
            ours: last.ours,
            theirs: last.theirs,
            count: conflicts.iter().map(|conflict| conflict.count).sum(),
        })
    })
    .flatten()
}

pub fn is_side(shared: &super::Shared, block: Uuid, commit: CommitId) -> bool {
    shared.conflicts.get(&block).is_some_and(|conflicts| {
        conflicts
            .iter()
            .any(|conflict| conflict.ours == commit || conflict.theirs == commit)
    })
}

pub fn dismiss(block: Uuid) {
    super::edit(vec![Edit::Dismiss(block)]);
    crate::changes::wake();
}

pub fn comparing() -> Option<Comparison> {
    super::with_seen(|shared| shared.comparison.clone()).flatten()
}

pub fn compare(block: Uuid) {
    let Some(conflicted) = conflicted(block) else {
        return;
    };
    super::edit(vec![Edit::Compare(block)]);
    for (side, commit) in [
        (Side::Ours, conflicted.ours),
        (Side::Theirs, conflicted.theirs),
    ] {
        super::revisions::request(
            block,
            super::revisions::Asked::Preview(*commit.hash().as_bytes()),
            move |reply| previewed(block, side, reply),
        );
    }
    crate::changes::wake();
}

fn previewed(block: Uuid, side: Side, reply: super::revisions::Answered) {
    let shown = match reply {
        super::revisions::Answered::Previewed(previewed) => previewed,
        _ => Err("This version could not be shown.".to_owned()),
    };
    super::edit(vec![Edit::Compared {
        block,
        theirs: side == Side::Theirs,
        shown,
    }]);
    crate::changes::wake();
}

pub fn stop_comparing() {
    if comparing().is_some() {
        super::edit(vec![Edit::StopComparing]);
    }
}

pub fn keep(block: Uuid, side: Side) {
    let Some(conflicted) = conflicted(block) else {
        return;
    };
    let commit = match side {
        Side::Ours => conflicted.ours,
        Side::Theirs => conflicted.theirs,
    };
    super::revisions::request(
        block,
        super::revisions::Asked::Restore(*commit.hash().as_bytes()),
        move |reply| match reply {
            super::revisions::Answered::Restored(Ok(())) => dismiss(block),
            super::revisions::Answered::Restored(Err(error)) => {
                super::edit(vec![Edit::KeepFailed { block, error }]);
                crate::changes::wake();
            }
            _ => {}
        },
    );
}
