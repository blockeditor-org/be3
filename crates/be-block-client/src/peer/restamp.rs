//! Stamping again a commit the server refused for its time (`ErrorCode::CommitAhead`): one
//! sealed by a clock running fast before this device measured the server's
//! (guides/blocks/client.md, "Saving and the outbox").

use std::collections::HashMap;

use be_block_commit::{COMMIT_CLOCK_SLACK, Commit, CommitId};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::Peer;
use crate::{
    ClientError,
    refs::{Op, Pending},
};

impl<S: ObjectStore> Peer<S> {
    /// Stamps the commits of `block` waiting in the outbox that are later than the server's
    /// clock as measured now lets this device stamp (`commit_time`) with a time it takes, and
    /// those built on them with their new parents. A commit's id is its hash, so each gets a
    /// new one, which the outbox, the local head, the journal and what announcements wait on
    /// move to (`Refs::restamp`); its change id stays. A time moves back by how far this
    /// device's clock is ahead of the server's, so the commits keep their order and spacing.
    /// Answers whether any commit was stamped again: with no measurement, or none past what
    /// the server takes, sending again would be refused again.
    pub(super) async fn restamp(&self, block: Uuid) -> Result<bool, ClientError> {
        let Some(server) = self.server_now() else {
            return Ok(false);
        };
        let latest = server.saturating_add(COMMIT_CLOCK_SLACK);
        let shift = server.saturating_sub(self.local_now());
        let waiting: Vec<(u64, Pending)> = self
            .refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(sequence, op)| match op {
                Op::Publish(pending) => Some((sequence, pending)),
                _ => None,
            })
            .collect();
        if !waiting.iter().any(|(_, pending)| pending.time > latest) {
            return Ok(false);
        }
        let key = self.sealing_key(block).await?;
        let mut renamed: HashMap<CommitId, CommitId> = HashMap::new();
        let mut times: HashMap<CommitId, i64> = HashMap::new();
        let mut rewritten = Vec::new();
        for (sequence, pending) in waiting {
            let rename = |id: CommitId| renamed.get(&id).copied().unwrap_or(id);
            let parents: Vec<CommitId> = pending.parents.iter().copied().map(rename).collect();
            let expected = pending.expected.map(rename);
            let ahead = pending.time > latest;
            if !ahead && parents == pending.parents {
                continue;
            }
            let mut time = match ahead {
                true => pending.time.saturating_add(shift.min(0)).min(latest),
                false => pending.time,
            };
            // Never before a commit it follows that was stamped again too.
            for parent in &parents {
                if let Some(parent) = times.get(parent) {
                    time = time.max(*parent);
                }
            }
            let chunks = pending
                .chunks
                .iter()
                .map(|hash| match renamed.get(&CommitId::from_hash(*hash)) {
                    Some(new) => new.hash(),
                    None => *hash,
                })
                .collect();
            let commit = self.commits.get(pending.commit)?;
            let change = commit.change_id(pending.commit);
            let id = self.commits.put(
                key,
                &Commit {
                    parents: parents.clone(),
                    time,
                    ..commit
                }
                .with_change(change),
            )?;
            renamed.insert(pending.commit, id);
            times.insert(id, time);
            rewritten.push((
                sequence,
                Pending {
                    commit: id,
                    expected,
                    parents,
                    chunks,
                    time,
                    ..pending
                },
            ));
        }
        self.commits.vault().store().sync()?;
        self.refs.restamp(block, &renamed, rewritten)?;
        Ok(true)
    }
}
