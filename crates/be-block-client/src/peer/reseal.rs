//! Sealing an edit made offline again: under a new epoch of its scope, when the logs show the
//! one it was sealed under outdated by the time it is sent (guides/blocks/keys.md, "Outdated
//! epochs"), or with a time the server takes, when it was stamped by a clock running fast
//! before this device measured the server's (guides/blocks/client.md, "Saving and the
//! outbox").

use std::collections::HashMap;

use be_block_commit::{COMMIT_CLOCK_SLACK, Commit, CommitId};
use be_block_store::{ObjectStore, Part};
use uuid::Uuid;

use super::{Peer, scope_of};
use crate::{
    ClientError,
    refs::{Op, Pending},
};

impl<S: ObjectStore> Peer<S> {
    /// Seals every commit of `block` waiting in the outbox again under the key the block is
    /// written under now, one this device vouches for (`sealing_key`, which gives the scope a
    /// new epoch first when the logs outdate the one at hand), each on the commits it went on
    /// as sealed again: the edit was this account's, only its key may be held by someone who
    /// lost access. False when it cannot be: this account may no longer write the block as the
    /// logs show now, or no epoch it can vouch for can be made. Offline, the error.
    pub(super) async fn reseal(&self, block: Uuid) -> Result<bool, ClientError> {
        if !self.may_write_signed(block).await? {
            return Ok(false);
        }
        let key = match self.sealing_key(block).await {
            Ok(key) => key,
            Err(error) if error.is_unreachable() => return Err(error),
            Err(_) => return Ok(false),
        };
        let summary = match self.refs.summary(block) {
            Some(summary) => summary,
            None => self.summary(block).await?,
        };
        if self.keys.outdated(key, scope_of(&summary)?) {
            return Ok(false);
        }
        self.seal_waiting_again(block, key, None).await
    }

    /// Stamps the commits of `block` waiting in the outbox again, when the server refused one
    /// for a time past its clock (`ErrorCode::CommitAhead`): sealed by a clock running fast
    /// before this device measured the server's. Each one too late moves back by as much as
    /// the newest must to be the latest the server takes, so they keep their order and
    /// spacing, and is never before a commit it follows; their change ids stay. False with no
    /// measurement of the server's clock, or none of them too late: sending again would be
    /// refused again.
    pub(super) async fn restamp(&self, block: Uuid) -> Result<bool, ClientError> {
        let Some(server) = self.server_now() else {
            return Ok(false);
        };
        let latest = server.saturating_add(COMMIT_CLOCK_SLACK);
        let Some(newest) = self
            .waiting_publishes(block)
            .iter()
            .map(|(_, pending)| pending.time)
            .max()
        else {
            return Ok(false);
        };
        if newest <= latest {
            return Ok(false);
        }
        let key = match self.sealing_key(block).await {
            Ok(key) => key,
            Err(error) if error.is_unreachable() => return Err(error),
            Err(_) => return Ok(false),
        };
        self.seal_waiting_again(
            block,
            key,
            Some(Retime {
                latest,
                back: newest - latest,
            }),
        )
        .await
    }

    fn waiting_publishes(&self, block: Uuid) -> Vec<(u64, Pending)> {
        self.refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(sequence, op)| match op {
                Op::Publish(pending) => Some((sequence, pending)),
                _ => None,
            })
            .collect()
    }

    /// Seals every commit of `block` waiting in the outbox again under `key`, each on the
    /// commits it went on as sealed again and `retime` moving its time (never before a commit it
    /// follows), and puts them in their places in one refs write, which keeps what replaced
    /// what. A seal of the block that lands meanwhile is among them next time round: the refs
    /// take the rewrite only while the outbox holds what it read (`Refs::replace_publishes`),
    /// and one that comes after goes on the replacements (`Refs::push_publish`). If seals keep
    /// landing through four tries, `Contended`, and the outbox is left as it is: the drain
    /// counts the block as failed this round and tries again on the next (`Drained::Failed`).
    async fn seal_waiting_again(
        &self,
        block: Uuid,
        key: Uuid,
        retime: Option<Retime>,
    ) -> Result<bool, ClientError> {
        for _ in 0..4 {
            let waiting = self.waiting_publishes(block);
            let mut moved: HashMap<CommitId, CommitId> = HashMap::new();
            let mut times: HashMap<CommitId, i64> = HashMap::new();
            let mut rewritten = Vec::with_capacity(waiting.len());
            for (sequence, pending) in waiting {
                // The commits it follows, as stamped again, or as this device holds them.
                let mut parents_at = None;
                for parent in &pending.parents {
                    let at = match moved.get(parent) {
                        Some(again) => times.get(again).copied(),
                        None => self.commits.try_get(*parent)?.map(|commit| commit.time),
                    };
                    parents_at = parents_at.max(at);
                }
                let time = match retime {
                    Some(retime) if pending.time > retime.latest => {
                        pending.time.saturating_sub(retime.back)
                    }
                    _ => pending.time,
                };
                let time = parents_at.map_or(time, |parent| time.max(parent));
                let again = self.reseal_pending(key, &pending, &moved, time).await?;
                moved.insert(pending.commit, again.commit);
                times.insert(again.commit, time);
                rewritten.push((sequence, pending.commit, again));
            }
            self.commits.vault().store().sync()?;
            if self.refs.replace_publishes(block, rewritten, &moved)? {
                return Ok(true);
            }
        }
        Err(ClientError::Contended)
    }

    /// The commits `commit` was sealed again in the place of (`reseal`, `restamp`), as the refs
    /// keep them across restarts, for a host whose replica kept them: they reach the server as
    /// it does.
    pub fn resealed_as(&self, commit: CommitId) -> Vec<CommitId> {
        self.refs.replaced_by(commit)
    }

    /// `pending`, its content, commit, metadata and journal state sealed again under `key`, on
    /// its parents as `moved` sealed them again, stamped `time`.
    async fn reseal_pending(
        &self,
        key: Uuid,
        pending: &Pending,
        moved: &HashMap<CommitId, CommitId>,
        time: i64,
    ) -> Result<Pending, ClientError> {
        let vault = self.commits.vault();
        let again = |commit: CommitId| moved.get(&commit).copied().unwrap_or(commit);
        let before = self.commits.get(pending.commit)?;
        let parts = before
            .manifest
            .root
            .map_or_else(|| Part::from(Vec::new()), Part::Reused);
        let manifest = self
            .write_content_under(key, before.manifest.content_type, parts)
            .await?;
        let expected = pending.expected.map(again);
        let parents: Vec<CommitId> = pending.parents.iter().copied().map(again).collect();
        let mut chunks = self.objects_of(&manifest).await?;
        for parent in parents.iter().skip(usize::from(expected.is_some())) {
            chunks.push(parent.hash());
            chunks.extend(
                self.objects_of(&self.commits.get(*parent)?.manifest)
                    .await?,
            );
        }
        // Its change stays the one it was, whatever its id becomes.
        let change = before.change_id(pending.commit);
        let commit = Commit {
            manifest,
            parents,
            time,
            ..before
        }
        .with_change(change);
        let id = self.commits.put(key, &commit)?;
        let journal = match &pending.journal {
            Some(seal) => {
                let mut seal = seal.clone();
                let state = vault.open(&seal.base.state)?;
                seal.base.state = vault.seal(key, &state)?;
                seal.base.commit = seal.base.commit.map(|base| match base == pending.commit {
                    true => id,
                    false => again(base),
                });
                Some(seal)
            }
            None => None,
        };
        Ok(Pending {
            commit: id,
            expected,
            parents: commit.parents,
            chunks,
            time,
            metadata: vault.seal(key, &commit.metadata)?,
            journal,
            ..pending.clone()
        })
    }
}

/// How `Peer::restamp` moves the times of commits stamped too late: those past `latest` go
/// back by `back`.
#[derive(Clone, Copy)]
struct Retime {
    latest: i64,
    back: i64,
}
