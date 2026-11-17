use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use be_block_commit::{CommitId, now_milliseconds};
use be_block_graph::{Access, BlockParent};
use be_block_protocol::{BlockSummary, ErrorCode, Listing, PublishOutcome};
use be_block_store::{Hash, ObjectStore};
use uuid::Uuid;

use super::{Drained, Peer, Reconciled};
use crate::{
    ClientError,
    refs::{Creation, Defining, Holding, Moving, Op, Pending, PendingCounts, UnsentBlock},
};

impl<S: ObjectStore> Peer<S> {
    pub fn pending(&self) -> PendingCounts {
        self.refs.counts()
    }

    /// What waits in the outbox, in the order the drain sends it: each operation's kind and
    /// block, for a host to name what keeps it from being in step.
    pub fn outbox(&self) -> Vec<(&'static str, Uuid)> {
        self.refs
            .outbox()
            .into_iter()
            .map(|(_, op)| {
                let kind = match op {
                    Op::Create(_) => "create",
                    Op::Publish(_) => "publish",
                    Op::Move(_) => "move",
                    Op::Define(_) => "define",
                    Op::Hold(_) => "hold",
                };
                (kind, op.block())
            })
            .collect()
    }

    pub fn has_pending(&self, block: Uuid) -> bool {
        self.refs.has_pending(block)
    }

    /// Whether what this device made of the block is set aside (`unsent`).
    pub fn is_unsent(&self, block: Uuid) -> bool {
        self.refs.is_unsent(block)
    }

    /// Why what this device made of `block` is set aside, when it was refused or outdated
    /// (`UnsentBlock::refused`).
    pub fn refusal(&self, block: Uuid) -> Option<crate::refs::Aside> {
        self.refs.refused(block)
    }

    /// Whether the block's creation waits in the outbox: the server does not know the block yet.
    pub fn creating(&self, block: Uuid) -> bool {
        self.refs.creating(block)
    }

    /// Whether the server does not have the block: its creation waits in the outbox, or was
    /// refused and set aside among the unsent edits, where it waits to be restored.
    pub fn uncreated(&self, block: Uuid) -> bool {
        self.refs.creating(block) || self.refs.creation_set_aside(block)
    }

    /// Whether a creation, move or definition of the block waits in the outbox, so what comes
    /// after it waits there too and goes in order.
    pub fn has_structural(&self, block: Uuid) -> bool {
        self.refs.has_structural(block)
    }

    /// Moves a block on this device and keeps the move in the outbox for the drain to send.
    pub fn keep_move(&self, block: Uuid, parent: BlockParent) -> Result<(), ClientError> {
        self.move_locally(block, parent)
    }

    pub fn pending_commits(&self, block: Uuid) -> Vec<CommitId> {
        self.refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(_, op)| match op {
                Op::Publish(pending) => Some(pending.commit),
                _ => None,
            })
            .collect()
    }

    pub(super) fn pending_publish(&self, block: Uuid, commit: CommitId) -> Option<(u64, Pending)> {
        self.refs
            .pending_for(block)
            .into_iter()
            .find_map(|(sequence, op)| match op {
                Op::Publish(pending) if pending.commit == commit => Some((sequence, pending)),
                _ => None,
            })
    }

    pub fn withdraw(
        &self,
        block: Uuid,
        commit: CommitId,
        before: Option<CommitId>,
    ) -> Result<(), ClientError> {
        self.abandon(block, commit)?;
        if self.local_head(block) == Some(commit) {
            self.refs.set_head_to(block, before)?;
        }
        Ok(())
    }

    pub fn known_summary(&self, block: Uuid) -> Option<BlockSummary> {
        self.refs.summary(block)
    }

    pub(super) fn move_locally(&self, block: Uuid, parent: BlockParent) -> Result<(), ClientError> {
        let Some(mut summary) = self.refs.summary(block) else {
            return Err(ClientError::Disconnected(format!(
                "block {block} is not saved on this device, so it cannot move offline"
            )));
        };
        let from = summary.parent;
        self.refs.push(Op::Move(Moving {
            block,
            parent,
            from,
        }))?;
        summary.parent = Some(parent);
        self.refs.revise(summary)?;
        self.settle_locally()?;
        Ok(())
    }

    /// Keeps a hold of `objects` for `block` in the outbox, which uploads and holds them once it
    /// is sent (`drain_hold`): kept on this device meanwhile.
    pub(super) fn hold_later(
        &self,
        block: Uuid,
        objects: &[Hash],
        hold: be_block_protocol::Hold,
    ) -> Result<(), ClientError> {
        self.commits.vault().store().sync()?;
        self.refs.push(Op::Hold(Holding {
            block,
            objects: objects.to_vec(),
            lasting: matches!(hold, be_block_protocol::Hold::Lasting),
        }))?;
        self.settle_locally()?;
        Ok(())
    }

    /// The block's history as far as this device holds it, from the server's head as last heard
    /// back through every parent it has: what `history` answers offline. A commit it never
    /// fetched ends the walk there.
    pub(super) fn history_held(
        &self,
        block: Uuid,
    ) -> Result<Vec<be_block_protocol::HistoryEntry>, ClientError> {
        let head = self
            .known_remote_head(block)
            .or_else(|| self.refs.summary(block).and_then(|summary| summary.head));
        // Each commit after its parents (a walk that puts a commit down once every parent it
        // holds is down), then oldest first, which keeps that order where two share a time.
        let mut entries = Vec::new();
        let mut done = HashSet::new();
        let mut stack: Vec<(CommitId, bool)> = head.into_iter().map(|id| (id, false)).collect();
        while let Some((id, parents_down)) = stack.pop() {
            if done.contains(&id) {
                continue;
            }
            let Some(commit) = self.commits.try_get(id)? else {
                done.insert(id);
                continue;
            };
            if !parents_down {
                stack.push((id, true));
                stack.extend(
                    commit
                        .parents
                        .iter()
                        .filter(|parent| !done.contains(*parent))
                        .map(|parent| (*parent, false)),
                );
                continue;
            }
            done.insert(id);
            entries.push(be_block_protocol::HistoryEntry {
                commit: id,
                parents: commit.parents,
                time: commit.time,
                pinned: matches!(commit.kind, be_block_commit::CommitKind::Bookmark(_)),
                head: Some(id) == head,
            });
        }
        entries.sort_by_key(|entry| entry.time);
        Ok(entries)
    }

    pub(super) fn define_locally(
        &self,
        mut summary: BlockSummary,
        metadata: Vec<u8>,
        references: Vec<Uuid>,
    ) -> Result<BlockSummary, ClientError> {
        self.refs.push(Op::Define(Defining {
            block: summary.id,
            metadata: metadata.clone(),
            references,
        }))?;
        summary.metadata = metadata;
        self.refs.revise(summary.clone())?;
        self.settle_locally()?;
        Ok(summary)
    }

    pub fn abandon(&self, block: Uuid, commit: CommitId) -> Result<(), ClientError> {
        if let Some((sequence, _)) = self.pending_publish(block, commit) {
            self.refs.remove(&[sequence])?;
            self.refs.commits_refused(block, &HashSet::from([commit]))?;
        }
        Ok(())
    }

    pub(super) fn settle(
        &self,
        sequence: u64,
        published: &Pending,
        head: CommitId,
    ) -> Result<(), ClientError> {
        let block = published.block;
        let waiting: Vec<(u64, Pending)> = self
            .refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(other, op)| match op {
                Op::Publish(pending) if other != sequence => Some((other, pending)),
                _ => None,
            })
            .collect();
        let mut settled = vec![sequence];
        let mut commits = HashSet::from([published.commit, head]);
        if !waiting.is_empty() {
            let targets: HashSet<CommitId> =
                waiting.iter().map(|(_, pending)| pending.commit).collect();
            let reached = self.ancestors_among(head, &targets)?;
            for (other, pending) in &waiting {
                if reached.contains(&pending.commit) {
                    settled.push(*other);
                    commits.insert(pending.commit);
                }
            }
        }
        self.refs.commits_held(block, &commits)?;
        self.refs
            .settle(block, &settled, head, self.behind(), |local| {
                local.is_none_or(|local| commits.contains(&local))
            })?;
        Ok(())
    }

    /// Whether a commit of `block` waits in the outbox ahead of `commit` (at `sequence`) that
    /// `commit` does not descend from: sent first, `commit` would leave it behind.
    pub(super) fn beside(
        &self,
        block: Uuid,
        sequence: u64,
        commit: CommitId,
    ) -> Result<bool, ClientError> {
        let ahead: HashSet<CommitId> = self
            .refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(other, op)| match op {
                Op::Publish(pending) if other < sequence => Some(pending.commit),
                _ => None,
            })
            .collect();
        if ahead.is_empty() {
            return Ok(false);
        }
        Ok(self.ancestors_among(commit, &ahead)?.len() < ahead.len())
    }

    fn ancestors_among(
        &self,
        head: CommitId,
        targets: &HashSet<CommitId>,
    ) -> Result<HashSet<CommitId>, ClientError> {
        let mut reached = HashSet::new();
        let mut seen = HashSet::new();
        let mut frontier = vec![head];
        while let Some(id) = frontier.pop() {
            if reached.len() == targets.len() {
                break;
            }
            if !seen.insert(id) {
                continue;
            }
            if targets.contains(&id) {
                reached.insert(id);
            }
            if let Some(commit) = self.commits.try_get(id)? {
                frontier.extend(commit.parents);
            }
        }
        Ok(reached)
    }

    pub(super) fn create_locally(
        &self,
        creation: Creation,
        made: crate::peer::MadeRoot<'_>,
    ) -> Result<BlockSummary, ClientError> {
        let summary = self.record_creation(&creation)?;
        self.refs.push(Op::Create(creation))?;
        made.kept()?;
        self.settle_locally()?;
        Ok(summary)
    }

    /// Keeps on the device the log made with `creation` (`new_log`), once the outbox keeps it.
    pub(super) fn keep_made_log(&self, creation: &Creation) -> Result<(), ClientError> {
        let Some(root) = crate::peer::made_root(creation) else {
            return Ok(());
        };
        if let Some(kept) = self.keys.kept_log(creation.block)
            && kept.head == root
        {
            self.refs.keep_log(creation.block, kept)?;
        }
        Ok(())
    }

    /// Lets go of the root and log made with `creation` (`new_log`), on the device and in
    /// memory, as no server took it (`KeyBook::forget_made`).
    pub(super) fn forget_made(&self, creation: &Creation) -> Result<(), ClientError> {
        let Some(root) = crate::peer::made_root(creation) else {
            return Ok(());
        };
        let minted: Vec<Uuid> = creation.keys.keys.iter().map(|row| row.id).collect();
        self.keys.forget_made(creation.block, root, &minted);
        self.refs.unmade_root(creation.block, root, &minted)?;
        Ok(())
    }

    /// A local account's outbox settled as a server would answer it (`Peer::local`): every
    /// creation, move, definition and hold is final as it was kept, and a commit is the block's
    /// head when it goes on the head the block has (`publish_locally`). One that does not waits
    /// as a divergence to merge, as the drain would find it, and what follows of its block
    /// waits behind it. Does nothing for a peer with a server.
    pub(super) fn settle_locally(&self) -> Result<Vec<Drained>, ClientError> {
        let mut outcomes = Vec::new();
        if !self.local {
            return Ok(outcomes);
        }
        let mut stopped = HashSet::new();
        for (sequence, op) in self.refs.outbox() {
            let block = op.block();
            if stopped.contains(&block) || !self.refs.holds(sequence) {
                continue;
            }
            let outcome = match op {
                Op::Publish(pending) => match self.publish_locally(&pending)? {
                    PublishOutcome::Published(head) => {
                        self.settle(sequence, &pending, head)?;
                        let mut objects = pending.chunks.clone();
                        objects.push(pending.commit.hash());
                        self.grow_locally(block, &objects)?;
                        // Its summary says the head too, as a server's listing would.
                        if let Some(mut summary) = self.refs.summary(block)
                            && summary.head != Some(head)
                        {
                            summary.head = Some(head);
                            self.refs.revise(summary)?;
                        }
                        Drained::Published { block, head }
                    }
                    PublishOutcome::Rejected(remote) => {
                        stopped.insert(block);
                        Drained::Diverged {
                            block,
                            local: self.local_head(block).unwrap_or(pending.commit),
                            remote,
                        }
                    }
                },
                Op::Create(_) => {
                    self.refs.remove(&[sequence])?;
                    Drained::Created { block }
                }
                Op::Move(moving) => {
                    self.refs.remove(&[sequence])?;
                    self.refs.carry_tree(block, moving.from, moving.parent)?;
                    Drained::Moved {
                        block,
                        from: moving.from,
                    }
                }
                Op::Define(_) => {
                    self.refs.remove(&[sequence])?;
                    Drained::Defined { block }
                }
                Op::Hold(holding) => {
                    self.refs.remove(&[sequence])?;
                    self.grow_locally(block, &holding.objects)?;
                    Drained::Held { block }
                }
            };
            if !matches!(outcome, Drained::Diverged { .. }) {
                self.settled_here.lock().unwrap().insert(block);
            }
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }

    /// A local account's `block` holds `objects` too: its size counts the bytes of those it did
    /// not hold yet, as a server counts the distinct objects a block holds (`Refs::grow`).
    fn grow_locally(&self, block: Uuid, objects: &[Hash]) -> Result<(), ClientError> {
        let unheld = self.refs.unheld_objects(block, objects);
        let store = self.commits.vault().store();
        let mut bytes = 0u64;
        for object in &unheld {
            bytes += store.len(*object)?.unwrap_or_default() as u64;
        }
        self.refs.grow(block, &unheld, bytes)?;
        Ok(())
    }

    /// What a server answers a publish, for a local account: taken when it goes on the block's
    /// head, a commit already reached by it settled, and otherwise refused with that head.
    pub(super) fn publish_locally(&self, pending: &Pending) -> Result<PublishOutcome, ClientError> {
        let head = self.known_remote_head(pending.block);
        if pending.expected == head {
            return Ok(PublishOutcome::Published(pending.commit));
        }
        match head {
            Some(head) if self.commits.is_ancestor(pending.commit, head)? => {
                Ok(PublishOutcome::Published(head))
            }
            head => Ok(PublishOutcome::Rejected(head)),
        }
    }

    pub(super) fn record_creation(&self, creation: &Creation) -> Result<BlockSummary, ClientError> {
        let scope = match creation.parent {
            BlockParent::Block(_) if creation.owner_only => Some(creation.block),
            BlockParent::Block(parent) => {
                self.refs.summary(parent).and_then(|summary| summary.scope)
            }
            BlockParent::Root | BlockParent::Detached => Some(creation.block),
        };
        let access = match creation.parent {
            BlockParent::Block(parent) if self.refs.summary(parent).is_some() => {
                let mut below_settings = false;
                let mut current = self.refs.summary(parent);
                let parent_access = current.as_ref().map_or(Access::None, |held| held.access);
                for _ in 0..256 {
                    let Some(held) = current else {
                        break;
                    };
                    let Some(up) = held.parent else {
                        break;
                    };
                    if be_block_graph::holds_settings(held.content_type, up) {
                        below_settings = true;
                        break;
                    }
                    current = up.block().and_then(|up| self.refs.summary(up));
                }
                be_block_graph::created_access(
                    creation.content_type,
                    creation.parent,
                    parent_access,
                    below_settings,
                )
            }
            BlockParent::Block(_) | BlockParent::Root | BlockParent::Detached => Access::Manage,
        };
        let summary = BlockSummary {
            id: creation.block,
            content_type: creation.content_type,
            access,
            author: Some(self.account),
            parent: Some(creation.parent),
            head: None,
            references: creation.references.clone(),
            metadata: creation.metadata.clone(),
            version: None,
            key: Some(creation.key),
            scope,
            changed: 0,
            // Its tree is its parent's, as far as this device knows.
            entry: match creation.parent {
                BlockParent::Block(parent) => {
                    self.refs.summary(parent).and_then(|parent| parent.entry)
                }
                BlockParent::Root | BlockParent::Detached => Some(creation.block),
            },
            held: true,
            opens_inside: false,
            sets_access: false,
            size: 0,
            tree_size: 0,
            owner_only: creation.owner_only,
        };
        self.refs.add_keys(&creation.keys)?;
        self.recorded(&creation.keys);
        self.refs
            .add_to_listing(Listing::Children(creation.parent), &summary)?;
        self.note(&summary);
        Ok(summary)
    }

    pub(super) async fn read_contents_cached(
        &self,
        blocks: &[(Uuid, Option<CommitId>)],
        reason: String,
    ) -> Result<Vec<super::BlockRead>, ClientError> {
        let mut read = Vec::with_capacity(blocks.len());
        for (block, _) in blocks {
            let mut summary = self
                .refs
                .summary(*block)
                .ok_or_else(|| ClientError::Disconnected(reason.clone()))?;
            summary.head = self.held_head(*block).or(summary.head);
            let content = match summary.head {
                Some(head) => Some(self.read_at(*block, head).await?),
                None => None,
            };
            let journal = self.local_journal(*block).ok().flatten();
            read.push(super::BlockRead {
                summary,
                stored: content,
                journal,
            });
        }
        Ok(read)
    }

    pub(super) async fn read_at(
        &self,
        block: Uuid,
        head: CommitId,
    ) -> Result<be_block_store::Stored, ClientError> {
        let commit = self.load_head(block, head).await?;
        self.load_manifest(&commit.manifest).await
    }

    /// Publishes the outbox in order (guides/blocks/client.md, "Saving and the outbox").
    /// Announcements the drain makes held wait for its end, so a burst goes as one: its caller
    /// sends them (`send_announcements`).
    pub async fn drain(&self, skip: impl Fn(Uuid) -> bool) -> Result<Vec<Drained>, ClientError> {
        let _draining = Draining::start(&self.draining);
        self.drain_outbox(skip).await
    }

    async fn drain_outbox(&self, skip: impl Fn(Uuid) -> bool) -> Result<Vec<Drained>, ClientError> {
        if self.local {
            return self.settle_locally();
        }
        let mut outcomes: Vec<Drained> = Vec::new();
        let mut stopped = HashSet::new();
        for (sequence, _) in self.refs.outbox() {
            // As it stands now: a commit sealed again (`reseal`) keeps its place.
            let Some(op) = self.refs.op(sequence) else {
                continue;
            };
            let block = op.block();
            if skip(block) || stopped.contains(&block) {
                continue;
            }
            // What was made since of a block the server refused to make joins it among the
            // unsent: the server would refuse it too, as a block it does not know.
            if self.refs.refused(block).is_some() {
                self.move_to_unsent(block, None)?;
                continue;
            }
            let mut publishing = match &op {
                Op::Publish(pending) => Some(pending.commit),
                _ => None,
            };
            let sent = match op {
                Op::Create(creation) => self.send_creation(sequence, creation).await,
                // Sealed again under a new epoch: sent once more as it now stands, and set
                // aside if that is refused as outdated too.
                Op::Publish(pending) => match self.drain_publish(sequence, pending, true).await {
                    Ok(resealed @ Drained::Resealed { .. }) => {
                        outcomes.push(resealed);
                        let Some(Op::Publish(pending)) = self.refs.op(sequence) else {
                            continue;
                        };
                        publishing = Some(pending.commit);
                        self.drain_publish(sequence, pending, false).await
                    }
                    sent => sent,
                },
                Op::Move(moving) => self.drain_move(sequence, moving).await,
                Op::Define(defining) => self.drain_define(sequence, defining).await,
                Op::Hold(holding) => self.drain_hold(sequence, holding).await,
            };
            let outcome = match sent {
                Ok(outcome) => outcome,
                Err(error) if error.is_unreachable() && outcomes.is_empty() => return Err(error),
                Err(error) if error.is_unreachable() => break,
                Err(ClientError::Refused(code, _)) => {
                    stopped.insert(block);
                    if let Some(commit) = publishing {
                        self.refs.commits_refused(block, &HashSet::from([commit]))?;
                    }
                    Drained::Refused {
                        block,
                        code,
                        restored: false,
                    }
                }
                Err(_) => {
                    stopped.insert(block);
                    Drained::Failed { block }
                }
            };
            match outcome {
                Drained::Published { .. } => {
                    outcomes.retain(|held| {
                        !matches!(held, Drained::Published { block: other, .. } if *other == block)
                    });
                }
                Drained::Diverged { .. } => {
                    stopped.insert(block);
                }
                Drained::Unsent { .. } => {
                    stopped.insert(block);
                }
                Drained::Created { .. }
                | Drained::Moved { .. }
                | Drained::Defined { .. }
                | Drained::Held { .. }
                | Drained::Refused { .. }
                | Drained::SetAside { .. }
                | Drained::Resealed { .. }
                | Drained::Failed { .. } => {}
            }
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }

    pub async fn reconcile(
        &self,
        block: Uuid,
        remote: Option<CommitId>,
    ) -> Result<Reconciled, ClientError> {
        let mut remote = remote;
        for _ in 0..4 {
            if let Some(theirs) = remote {
                self.fetch_history(theirs).await?;
                self.settle_reached(block, theirs)?;
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
            let Some((_, newest)) = waiting.last() else {
                return Ok(Reconciled::Done);
            };
            let ours = match self.local_head(block) {
                Some(local) if waiting.iter().any(|(_, pending)| pending.commit == local) => local,
                _ => newest.commit,
            };
            if let Some(theirs) = remote
                && !self.commits.is_ancestor(theirs, ours)?
            {
                let base = match self.commits.common_ancestor(ours, theirs)? {
                    Some(base) => Some(base),
                    None => self.left_from(&waiting)?,
                };
                return Ok(Reconciled::Merge { base, ours, theirs });
            }
            let Some((sequence, pending)) = waiting
                .iter()
                .find(|(_, pending)| pending.expected == remote)
            else {
                return Ok(match remote {
                    Some(theirs) => Reconciled::Merge {
                        base: Some(theirs),
                        ours,
                        theirs,
                    },
                    None => Reconciled::Done,
                });
            };
            match self.send_pending(pending).await? {
                PublishOutcome::Published(head) => {
                    self.settle(*sequence, pending, head)?;
                    remote = Some(head);
                }
                PublishOutcome::Rejected(head) => remote = head,
            }
        }
        Err(ClientError::Contended)
    }

    /// The remote head the waiting commits were built on, which this device holds: the base of
    /// their merge when the server pruned the history between it and the new head, so no common
    /// ancestor can be walked to.
    fn left_from(&self, waiting: &[(u64, Pending)]) -> Result<Option<CommitId>, ClientError> {
        for (_, pending) in waiting.iter().rev() {
            let Some(expected) = pending.expected else {
                continue;
            };
            if waiting.iter().any(|(_, other)| other.commit == expected) {
                continue;
            }
            if self.commits.has(expected)? {
                return Ok(Some(expected));
            }
        }
        Ok(None)
    }

    fn settle_reached(&self, block: Uuid, head: CommitId) -> Result<(), ClientError> {
        let waiting: Vec<(u64, CommitId)> = self
            .refs
            .pending_for(block)
            .into_iter()
            .filter_map(|(sequence, op)| match op {
                Op::Publish(pending) => Some((sequence, pending.commit)),
                _ => None,
            })
            .collect();
        let targets: HashSet<CommitId> = waiting.iter().map(|(_, commit)| *commit).collect();
        let reached = self.ancestors_among(head, &targets)?;
        let settled: Vec<u64> = waiting
            .iter()
            .filter(|(_, commit)| reached.contains(commit))
            .map(|(sequence, _)| *sequence)
            .collect();
        self.refs.commits_held(block, &reached)?;
        self.refs
            .settle(block, &settled, head, self.behind(), |local| {
                local.is_none_or(|local| local == head || reached.contains(&local))
            })?;
        Ok(())
    }

    /// Whether `creation`, made offline under the scope its kept summary named
    /// (`Creation::unchecked`), wraps a key under the key of a block its parent does not lie at
    /// or under by the signed parent records (`Peer::signed_below`), or one it cannot tell for a
    /// root it never pinned. A read that does not get through is a failure to send, tried again
    /// later.
    async fn misplaced(&self, creation: &Creation) -> Result<bool, ClientError> {
        let BlockParent::Block(parent) = creation.parent else {
            return Ok(false);
        };
        let scopes: HashSet<Uuid> = creation
            .keys
            .wrapped
            .iter()
            .filter_map(|wrap| match wrap.wrapped_by {
                // The escrow key is the organization's, under no block.
                be_block_protocol::WrappedBy::Key(key)
                    if self
                        .keys
                        .row(key)
                        .is_some_and(|row| row.kind == be_block_protocol::KeyKind::Scope) =>
                {
                    self.keys.block_of(key)
                }
                _ => None,
            })
            .filter(|scope| *scope != creation.block)
            .collect();
        for scope in scopes {
            match self.signed_below(parent, scope).await {
                Ok(()) => {}
                // A root this device never pinned stays so: waiting would hold the outbox.
                Err(ClientError::Forged(_) | ClientError::Locked(_)) => return Ok(true),
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }

    async fn send_creation(
        &self,
        sequence: u64,
        creation: Creation,
    ) -> Result<Drained, ClientError> {
        let block = creation.block;
        // Made under a block the server refused to make: it cannot be made either, and waits
        // set aside with it, restored with it (`restore_unsent`).
        if let BlockParent::Block(parent) = creation.parent
            && let Some(why) = self.refs.refused(parent)
        {
            self.move_to_unsent(block, Some(why))?;
            return Ok(Drained::SetAside { block, why });
        }
        // Made offline under the scope a kept summary named: its keys go up only wrapped under
        // the key of a block the signed parent records place it under.
        if creation.unchecked && self.misplaced(&creation).await? {
            let why = crate::refs::Aside::Misplaced;
            self.move_to_unsent(block, Some(why))?;
            return Ok(Drained::SetAside { block, why });
        }
        let response = crate::ask(
            be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Op::CreateBlock {
                    block: block,
                    content_type: creation.content_type,
                    parent: creation.parent,
                    metadata: creation.metadata.clone(),
                    references: creation.references.clone(),
                    key: creation.key,
                    keys: creation.keys.clone(),
                    owner_only: creation.owner_only,
                    root: creation.root.clone(),
                    placement: creation.placement.clone(),
                },
            ),
        )
        .await;
        match response {
            Ok(summary) => {
                self.refs.remove(&[sequence])?;
                self.keys.made_taken(block);
                self.note(&summary);
                self.escrow_created(&creation.keys).await;
                Ok(Drained::Created { block })
            }
            // Made by this device already, its answer lost; or by someone else, as one this
            // device opened offline with no summary of it (`ensure_type`): what was made here of
            // it is not the block, and is set aside, its root and log let go.
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
                if self.made_here(&creation).await? {
                    self.refs.remove(&[sequence])?;
                    self.keys.made_taken(block);
                    return Ok(Drained::Created { block });
                }
                let why = crate::refs::Aside::Refused(ErrorCode::BlockAlreadyExists);
                self.forget_made(&creation)?;
                self.move_to_unsent(block, Some(why))?;
                // The block as the server has it, in place of what this device made of it. Its
                // answer is not needed here: failing, the block's next listing or summary reads
                // it, and the creation is set aside either way.
                let _ = self.server_summary(block).await;
                Ok(Drained::SetAside { block, why })
            }
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            // The server will not make the block as it is (over a quota, too large): what this
            // device made of it is set aside, to be restored or discarded, so nothing waits
            // behind a creation that never goes.
            Err(ClientError::Refused(code, _)) => {
                self.move_to_unsent(block, Some(crate::refs::Aside::Refused(code)))?;
                Ok(Drained::SetAside {
                    block,
                    why: crate::refs::Aside::Refused(code),
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Whether the block the server has under `creation`'s id is one this account made: its
    /// log starts at the root made with it, which every device of the account makes the same
    /// (`Entry::root` signs the account and the block alone), so the first of two devices
    /// counts. A block this account may not read the log of is someone else's.
    pub(super) async fn made_here(&self, creation: &Creation) -> Result<bool, ClientError> {
        let Some(root) = crate::peer::made_root(creation) else {
            return Ok(false);
        };
        match self.served_log(creation.block).await {
            Ok(steps) => Ok(steps.first().is_some_and(|first| first.hash() == root)),
            // Deliberately someone else's: this account may read the log of any block it made,
            // so one it may not read is not one it made.
            Err(ClientError::Refused(code, _)) if lost_access(code) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn drain_move(&self, sequence: u64, moving: Moving) -> Result<Drained, ClientError> {
        let block = moving.block;
        match self.send_parent(block, moving.parent).await {
            Ok(()) => {
                self.refs.remove(&[sequence])?;
                let _ = self
                    .follow_prefixes(block, moving.from, moving.parent)
                    .await;
                Ok(Drained::Moved {
                    block,
                    from: moving.from,
                })
            }
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            Err(ClientError::Refused(code, _)) => {
                self.refs.remove(&[sequence])?;
                self.unchanged(block).await;
                Ok(Drained::Refused {
                    block,
                    code,
                    restored: true,
                })
            }
            Err(error) => Err(error),
        }
    }

    async fn drain_hold(&self, sequence: u64, holding: Holding) -> Result<Drained, ClientError> {
        let block = holding.block;
        let hold = match holding.lasting {
            true => be_block_protocol::Hold::Lasting,
            false => be_block_protocol::Hold::UntilCommitted,
        };
        match self.send_hold(block, &holding.objects, hold).await {
            Ok(()) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Held { block })
            }
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            // As a publish is: set aside only when sending it again cannot help, and otherwise
            // kept to try again (a quota, a missing key), since a hold dropped could let what a
            // commit still to publish uses be collected first.
            Err(ClientError::Refused(code, _)) if never_taken(code) => {
                self.move_to_unsent(block, Some(crate::refs::Aside::Refused(code)))?;
                Ok(Drained::SetAside {
                    block,
                    why: crate::refs::Aside::Refused(code),
                })
            }
            Err(error) => Err(error),
        }
    }

    async fn drain_define(
        &self,
        sequence: u64,
        defining: Defining,
    ) -> Result<Drained, ClientError> {
        let block = defining.block;
        let response = crate::ask(
            be_block_protocol::write::<be_block_protocol::BlockSummary, _>(
                &self.connection(),
                Some(self.organization),
                be_block_protocol::Op::SetMetadata {
                    block: block,
                    metadata: defining.metadata.clone(),
                    references: defining.references.clone(),
                },
            ),
        )
        .await;
        match response {
            Ok(summary) => {
                self.refs.remove(&[sequence])?;
                self.note(&summary);
                Ok(Drained::Defined { block })
            }
            Err(ClientError::Refused(ErrorCode::BlockHasContent, _)) => {
                self.refs.remove(&[sequence])?;
                let plain = self
                    .commits
                    .vault()
                    .open(&defining.metadata)
                    .map_err(super::locked)?;
                let defined = be_block_content::BlockMetadata::decode(&plain)?.defined;
                self.define(block, |metadata| metadata.defined = defined.clone())
                    .await?;
                Ok(Drained::Defined { block })
            }
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            Err(ClientError::Refused(code, _)) => {
                self.refs.remove(&[sequence])?;
                self.unchanged(block).await;
                Ok(Drained::Refused {
                    block,
                    code,
                    restored: true,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Sends a commit waiting in the outbox. With `reseal`, one sealed offline under a key the
    /// logs now show outdated is sealed again under a new epoch (`Drained::Resealed`), to be
    /// sent as it then stands; otherwise, or when it cannot be, it is set aside.
    async fn drain_publish(
        &self,
        sequence: u64,
        pending: Pending,
        reseal: bool,
    ) -> Result<Drained, ClientError> {
        let block = pending.block;
        match self.send_pending(&pending).await {
            Ok(PublishOutcome::Published(head)) => {
                self.settle(sequence, &pending, head)?;
                Ok(Drained::Published { block, head })
            }
            Ok(PublishOutcome::Rejected(Some(head)))
                if self.contains(head, pending.commit).await? =>
            {
                self.settle(sequence, &pending, head)?;
                Ok(Drained::Published { block, head })
            }
            Ok(PublishOutcome::Rejected(head)) => Ok(Drained::Diverged {
                block,
                local: self.local_head(block).unwrap_or(pending.commit),
                remote: head,
            }),
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            // The server will never take this commit as it is: what this device made of the
            // block is set aside with the reason, to be restored or discarded, rather than
            // sent again every round.
            Err(ClientError::Refused(code, _)) if never_taken(code) => {
                self.move_to_unsent(block, Some(crate::refs::Aside::Refused(code)))?;
                Ok(Drained::SetAside {
                    block,
                    why: crate::refs::Aside::Refused(code),
                })
            }
            // Sealed offline under a key the log now shows outdated (`send_pending`): the edit
            // is this account's and only its key may be held by someone who lost access, so it
            // is sealed again under a new epoch this device vouches for. Set
            // aside, never sent, when this account may no longer write the block as the logs
            // show now, or no such epoch can be made.
            Err(ClientError::Outdated(_)) => {
                if reseal && self.reseal(block).await? {
                    return Ok(Drained::Resealed { block });
                }
                self.move_to_unsent(block, Some(crate::refs::Aside::KeyOutdated))?;
                Ok(Drained::SetAside {
                    block,
                    why: crate::refs::Aside::KeyOutdated,
                })
            }
            Err(error) => Err(error),
        }
    }

    async fn contains(&self, head: CommitId, commit: CommitId) -> Result<bool, ClientError> {
        if head == commit {
            return Ok(true);
        }
        self.fetch_history(head).await?;
        Ok(self
            .ancestors_among(head, &HashSet::from([commit]))?
            .contains(&commit))
    }

    async fn refused(
        &self,
        sequence: u64,
        block: Uuid,
        code: ErrorCode,
    ) -> Result<Drained, ClientError> {
        let summary = match self.server_summary(block).await {
            Ok(summary) => summary,
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                return self.lose_access(block);
            }
            Err(error) => return Err(error),
        };
        if !summary.access.can_edit() {
            self.move_to_unsent(block, None)?;
            return Ok(Drained::Unsent { block });
        }
        match self
            .refs
            .outbox()
            .into_iter()
            .find(|(held, _)| *held == sequence)
        {
            Some((_, Op::Create(_))) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Created { block })
            }
            Some((_, Op::Hold(_))) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Refused {
                    block,
                    code,
                    restored: false,
                })
            }
            Some((_, Op::Move(_) | Op::Define(_))) => {
                self.refs.remove(&[sequence])?;
                self.unchanged(block).await;
                Ok(Drained::Refused {
                    block,
                    code,
                    restored: true,
                })
            }
            Some((_, Op::Publish(_))) | None => Err(ClientError::Refused(
                code,
                format!("block {block} refused a change this account may otherwise make"),
            )),
        }
    }

    /// After the server refused a move or a definition: this device shows the block as the
    /// server has it again, not with the change it refused (its name, its place in a listing).
    async fn unchanged(&self, block: Uuid) {
        if self.refs.has_structural(block) {
            return;
        }
        if let Ok(summary) = self.server_summary(block).await {
            let _ = self.refs.revise(summary);
        }
    }

    pub fn forget_offline(&self, keep: impl Fn(Uuid) -> bool) -> Result<usize, ClientError> {
        let mut forgotten = 0;
        for summary in self.refs.summaries() {
            if summary.content_type == be_block::WORKSPACE_TYPE
                && summary.parent == Some(BlockParent::Root)
                && !keep(summary.id)
            {
                forgotten += self.refs.forget(&self.refs.below(summary.id))?;
            }
        }
        Ok(forgotten)
    }

    pub fn forget_cached(&self, block: Uuid) -> Result<usize, ClientError> {
        let mut blocks = self.refs.below(block);
        blocks.insert(block);
        Ok(self.refs.forget(&blocks)?)
    }

    /// Sets what this device keeps of `block` aside: what its commits announce is never sent
    /// (a message shown "Not sent" tells nobody, and one sent again tells as a new one).
    /// With `refused`, the server refused it with that code; otherwise this account lost access.
    pub fn move_to_unsent(
        &self,
        block: Uuid,
        refused: Option<crate::refs::Aside>,
    ) -> Result<bool, ClientError> {
        let commits: HashSet<CommitId> = self.pending_commits(block).into_iter().collect();
        self.refs.commits_refused(block, &commits)?;
        Ok(self
            .refs
            .move_to_unsent(block, now_milliseconds(), refused)?)
    }

    fn lose_access(&self, block: Uuid) -> Result<Drained, ClientError> {
        self.refs.lose_access(block)?;
        self.move_to_unsent(block, None)?;
        Ok(Drained::Unsent { block })
    }

    pub fn unsent(&self) -> Vec<UnsentBlock> {
        self.refs
            .unsent()
            .into_iter()
            .map(|(block, unsent)| UnsentBlock {
                block,
                head: unsent.head,
                commits: unsent
                    .ops
                    .iter()
                    .filter_map(|op| match op {
                        Op::Publish(pending) => Some(pending.commit),
                        _ => None,
                    })
                    .collect(),
                since: unsent.since,
                offerable: self
                    .refs
                    .summary(block)
                    .is_some_and(|summary| summary.access.can_edit()),
                name: self
                    .refs
                    .summary(block)
                    .and_then(|summary| self.metadata(&summary).ok())
                    .and_then(|metadata| metadata.name()),
                refused: unsent.refused,
                kept_edits: self
                    .refs
                    .open_edits(block)
                    .map_or(0, |kept| kept.edits.len()),
            })
            .collect()
    }

    /// Puts what was set aside of `block` back in the outbox, with what was made under it and
    /// set aside for its refusal: the blocks restored, none when nothing was set aside.
    pub fn restore_unsent(&self, block: Uuid) -> Result<Vec<Uuid>, ClientError> {
        if !self.refs.restore_unsent(block)? {
            return Ok(Vec::new());
        }
        let mut restored = vec![block];
        let mut next = 0;
        while let Some(parent) = restored.get(next).copied() {
            next += 1;
            for (child, unsent) in self.refs.unsent() {
                let under = self
                    .refs
                    .summary(child)
                    .is_some_and(|summary| summary.parent == Some(BlockParent::Block(parent)));
                if under && unsent.refused.is_some() && self.refs.restore_unsent(child)? {
                    restored.push(child);
                }
            }
        }
        Ok(restored)
    }

    /// Discards what was set aside of `block`. Set aside because this account lost access, the
    /// local edits kept beside it go too: they can never reach the block, and kept they would
    /// keep it from being forgotten.
    pub fn discard_unsent(&self, block: Uuid) -> Result<bool, ClientError> {
        let Some(unsent) = self.refs.discard_unsent(block)? else {
            return Ok(false);
        };
        // A creation discarded unsent never reached a server: the root and log made with it
        // (`new_log`) are no block's, and kept they would refuse making it again.
        for op in &unsent.ops {
            if let Op::Create(creation) = op
                && creation.block == block
            {
                self.forget_made(creation)?;
            }
        }
        Ok(true)
    }
}

/// Counts a drain under way for as long as it lives, so one dropped mid-way (a canceled
/// future) or overlapping another still ends.
struct Draining<'a>(&'a AtomicUsize);

impl<'a> Draining<'a> {
    fn start(draining: &'a AtomicUsize) -> Self {
        draining.fetch_add(1, Ordering::SeqCst);
        Self(draining)
    }
}

impl Drop for Draining<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) fn unreachable(error: &ClientError) -> bool {
    error.is_unreachable()
}

pub(super) fn lost_access(code: ErrorCode) -> bool {
    matches!(code, ErrorCode::PermissionDenied | ErrorCode::BlockNotFound)
}

/// A refusal sending the same change again cannot get past: the change itself is wrong (a
/// malformed request, an object over the size limit, a time too far ahead), where a quota, a
/// missing key or the store may come right by the next try.
fn never_taken(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::InvalidRequest | ErrorCode::ObjectTooLarge | ErrorCode::CommitAhead
    )
}
