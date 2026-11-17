//! Sealing an edit made offline again under a new epoch of its scope, when the logs show the
//! one it was sealed under outdated by the time it is sent (plans/signed_access.md, step 7).

use std::collections::HashMap;

use be_block_commit::{Commit, CommitId};
use be_block_keys::{Level, LogKind};
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
        if !self.may_still_write(block).await? {
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
        let waiting: Vec<(u64, Pending)> = self
            .refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(sequence, op)| match op {
                Op::Publish(pending) => Some((sequence, pending)),
                _ => None,
            })
            .collect();
        let mut moved: HashMap<CommitId, CommitId> = HashMap::new();
        let mut rewritten = Vec::with_capacity(waiting.len());
        for (sequence, pending) in waiting {
            let again = self.reseal_pending(key, &pending, &moved).await?;
            moved.insert(pending.commit, again.commit);
            rewritten.push((sequence, again));
        }
        self.commits.vault().store().sync()?;
        self.refs.replace_publishes(block, rewritten, &moved)?;
        let mut resealed = self.resealed.lock().unwrap();
        // A write a replica bases on a commit sealed again goes on the one in its place.
        for (before, after) in &moved {
            resealed.insert(*before, *after);
        }
        Ok(true)
    }

    /// The commits `commit` was sealed again in the place of in this run (`reseal`), for a
    /// host whose replica kept them: they reach the server as it does.
    pub fn resealed_as(&self, commit: CommitId) -> Vec<CommitId> {
        let resealed = self.resealed.lock().unwrap();
        let mut found = vec![commit];
        let mut next = 0;
        while let Some(after) = found.get(next).copied() {
            next += 1;
            found.extend(
                resealed
                    .iter()
                    .filter(|(_, into)| **into == after)
                    .map(|(before, _)| *before),
            );
        }
        found.remove(0);
        found
    }

    /// The commit sealed again in `commit`'s place (`reseal`), or `commit` itself.
    pub(super) fn resealed(&self, commit: CommitId) -> CommitId {
        let resealed = self.resealed.lock().unwrap();
        let mut commit = commit;
        while let Some(after) = resealed.get(&commit) {
            commit = *after;
        }
        commit
    }

    /// `pending`, its content, commit, metadata and journal state sealed again under `key`, on
    /// its parents as `moved` sealed them again.
    async fn reseal_pending(
        &self,
        key: Uuid,
        pending: &Pending,
        moved: &HashMap<CommitId, CommitId>,
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
            chunks.extend(self.objects_of(&self.commits.get(*parent)?.manifest).await?);
        }
        let commit = Commit {
            manifest,
            parents,
            ..before
        };
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
            metadata: vault.seal(key, &commit.metadata)?,
            journal,
            ..pending.clone()
        })
    }

    /// Whether this account may still write `block` as its signed logs show now: Edit or above
    /// in the block's own log, or from outside it (administering the organization, or a block
    /// above). A log that does not check, or is not served, shows nothing. Offline, the error.
    async fn may_still_write(&self, block: Uuid) -> Result<bool, ClientError> {
        let checked = match self.checked_access(block).await {
            Ok((_, checked)) => checked,
            Err(error) if error.is_unreachable() => return Err(error),
            Err(_) => return Ok(false),
        };
        let me = self.account.into_bytes();
        if checked.holding(LogKind::Block, me, Level::Edit).is_some() {
            return Ok(true);
        }
        match self.authority_outside(block, &checked, Level::Edit).await {
            Ok(_) => Ok(true),
            Err(error) if error.is_unreachable() => Err(error),
            Err(_) => Ok(false),
        }
    }
}
