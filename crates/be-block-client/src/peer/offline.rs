use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use be_block_commit::{CommitId, now_milliseconds};
use be_block_graph::{Access, BlockParent};
use be_block_protocol::{BlockSummary, ErrorCode, Listing, client};
use be_block_store::ObjectStore;
use uuid::Uuid;

use super::{Drained, Peer, Reconciled, Saved};
use crate::{
    ClientError,
    refs::{Creation, Defining, Moving, Op, Pending, PendingCounts, UnsentBlock},
};

impl<S: ObjectStore> Peer<S> {
    pub fn pending(&self) -> PendingCounts {
        self.refs.counts()
    }

    pub fn has_pending(&self, block: Uuid) -> bool {
        self.refs.has_pending(block)
    }

    /// Whether what this device made of the block is set aside (`unsent`).
    pub fn is_unsent(&self, block: Uuid) -> bool {
        self.refs.is_unsent(block)
    }

    /// The code the server refused to make `block` with, while what this device made of it is
    /// set aside for that (`UnsentBlock::refused`).
    pub fn refusal(&self, block: Uuid) -> Option<ErrorCode> {
        self.refs.refused(block)
    }

    /// Whether the block's creation waits in the outbox: the server does not know the block yet.
    pub fn creating(&self, block: Uuid) -> bool {
        self.refs.creating(block)
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
        Ok(())
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
        superseding: bool,
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
                let superseded =
                    superseding && *other < sequence && pending.expected == published.expected;
                if reached.contains(&pending.commit) || superseded {
                    settled.push(*other);
                    commits.insert(pending.commit);
                }
            }
        }
        self.refs.commits_held(block, &commits)?;
        self.refs.settle(block, &settled, head, self.behind(), |local| {
            local.is_none_or(|local| commits.contains(&local))
        })?;
        Ok(())
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

    pub(super) fn create_locally(&self, creation: Creation) -> Result<BlockSummary, ClientError> {
        let summary = self.record_creation(&creation)?;
        self.refs.push(Op::Create(creation))?;
        Ok(summary)
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
            revision: 0,
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
                Some(head) => Some(self.read_at(head).await?),
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
        head: CommitId,
    ) -> Result<be_block_store::Stored, ClientError> {
        let commit = self.load_commit(head).await?;
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
        let mut outcomes: Vec<Drained> = Vec::new();
        let mut stopped = HashSet::new();
        for (sequence, op) in self.refs.outbox() {
            let block = op.block();
            if skip(block) || stopped.contains(&block) || !self.refs.holds(sequence) {
                continue;
            }
            // What was made since of a block the server refused to make joins it among the
            // unsent: the server would refuse it too, as a block it does not know.
            if self.refs.refused(block).is_some() {
                self.move_to_unsent(block, None)?;
                continue;
            }
            let publishing = match &op {
                Op::Publish(pending) => Some(pending.commit),
                _ => None,
            };
            let sent = match op {
                Op::Create(creation) => self.send_creation(sequence, creation).await,
                Op::Publish(pending) => self.drain_publish(sequence, pending).await,
                Op::Move(moving) => self.drain_move(sequence, moving).await,
                Op::Define(defining) => self.drain_define(sequence, defining).await,
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
                | Drained::Refused { .. }
                | Drained::SetAside { .. }
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
                Saved::Published(head) | Saved::Unchanged(head) => {
                    self.settle(*sequence, pending, head, true)?;
                    remote = Some(head);
                }
                Saved::Rejected { head } => remote = head,
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
        self.refs.settle(block, &settled, head, self.behind(), |local| {
            local.is_none_or(|local| local == head || reached.contains(&local))
        })?;
        Ok(())
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
            && let Some(code) = self.refs.refused(parent)
        {
            self.move_to_unsent(block, Some(code))?;
            return Ok(Drained::SetAside { block, code });
        }
        let response = crate::ask(client::blocks::create_block(
            &self.connection(),
            block,
            creation.content_type,
            creation.parent,
            creation.metadata.clone(),
            creation.references.clone(),
            creation.key,
            creation.keys.clone(),
            creation.owner_only,
        ))
        .await;
        match response {
            Ok(summary) => {
                self.refs.remove(&[sequence])?;
                self.note(&summary);
                Ok(Drained::Created { block })
            }
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Created { block })
            }
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
            }
            // The server will not make the block as it is (over a quota, too large): what this
            // device made of it is set aside, to be restored or discarded, so nothing waits
            // behind a creation that never goes.
            Err(ClientError::Refused(code, _)) => {
                self.move_to_unsent(block, Some(code))?;
                Ok(Drained::SetAside { block, code })
            }
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

    async fn drain_define(
        &self,
        sequence: u64,
        defining: Defining,
    ) -> Result<Drained, ClientError> {
        let block = defining.block;
        let response = crate::ask(client::blocks::set_metadata(
            &self.connection(),
            block,
            defining.metadata.clone(),
            defining.references.clone(),
        ))
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
                let defined = be_block::BlockMetadata::decode(&plain)?.defined;
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

    async fn drain_publish(&self, sequence: u64, pending: Pending) -> Result<Drained, ClientError> {
        let block = pending.block;
        match self.send_pending(&pending).await {
            Ok(Saved::Published(head) | Saved::Unchanged(head)) => {
                self.settle(sequence, &pending, head, false)?;
                Ok(Drained::Published { block, head })
            }
            Ok(Saved::Rejected { head: Some(head) })
                if self.contains(head, pending.commit).await? =>
            {
                self.settle(sequence, &pending, head, false)?;
                Ok(Drained::Published { block, head })
            }
            Ok(Saved::Rejected { head }) => Ok(Drained::Diverged {
                block,
                local: self.local_head(block).unwrap_or(pending.commit),
                remote: head,
            }),
            Err(ClientError::Refused(code, _)) if lost_access(code) => {
                self.refused(sequence, block, code).await
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
        refused: Option<ErrorCode>,
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

    pub fn discard_unsent(&self, block: Uuid) -> Result<bool, ClientError> {
        Ok(self.refs.take_unsent(block)?.is_some())
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
