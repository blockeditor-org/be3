use be_commit::CommitId;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Ours,
    Theirs,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
    super::with_shared(|shared| {
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
    stop_comparing();
    super::with_shared_mut(|shared| {
        if shared.conflicts.remove(&block).is_some() {
            shared.stale_revision += 1;
            shared.touch(block);
        }
    });
    block_app_base::host::wake();
}

pub fn comparing() -> Option<Comparison> {
    super::with_shared(|shared| shared.comparison.clone()).flatten()
}

pub fn compare(block: Uuid) {
    let Some(conflicted) = conflicted(block) else {
        return;
    };
    stop_comparing();
    super::with_shared_mut(|shared| {
        shared.comparison = Some(Comparison {
            block,
            ours: None,
            theirs: None,
            error: None,
        });
        shared.stale_revision += 1;
    });
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
    block_app_base::host::wake();
}

fn previewed(block: Uuid, side: Side, reply: super::revisions::Answered) {
    let shown = match reply {
        super::revisions::Answered::Previewed(previewed) => previewed,
        _ => Err("This version could not be shown.".to_owned()),
    };
    let kept = super::with_shared_mut(|shared| {
        let comparison = shared
            .comparison
            .as_mut()
            .filter(|comparison| comparison.block == block)?;
        let slot = match side {
            Side::Ours => &mut comparison.ours,
            Side::Theirs => &mut comparison.theirs,
        };
        *slot = Some(shown.clone());
        shared.stale_revision += 1;
        Some(())
    })
    .flatten()
    .is_some();
    if !kept && let Ok(preview) = shown {
        super::revisions::release(preview);
    }
    block_app_base::host::wake();
}

pub fn stop_comparing() {
    let Some(comparison) = super::with_shared_mut(|shared| {
        let taken = shared.comparison.take();
        if taken.is_some() {
            shared.stale_revision += 1;
        }
        taken
    })
    .flatten() else {
        return;
    };
    for preview in [comparison.ours, comparison.theirs]
        .into_iter()
        .flatten()
        .flatten()
    {
        super::revisions::release(preview);
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
                super::with_shared_mut(|shared| {
                    if let Some(comparison) = shared
                        .comparison
                        .as_mut()
                        .filter(|comparison| comparison.block == block)
                    {
                        comparison.error = Some(error);
                        shared.stale_revision += 1;
                    }
                });
                block_app_base::host::wake();
            }
            _ => {}
        },
    );
}
