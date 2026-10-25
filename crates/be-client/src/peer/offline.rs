use std::collections::HashSet;

use be_commit::{CommitId, now_milliseconds};
use be_graph::{Access, BlockParent};
use be_protocol::{BlockSummary, ClientMessage, ErrorCode, Listing, ServerMessage};
use be_store::ObjectStore;
use uuid::Uuid;

use super::{Drained, Peer, Saved};
use crate::{
    ClientError,
    refs::{Creation, Defining, Moving, Op, Pending, PendingCounts, UnsentBlock},
};

impl<S: ObjectStore> Peer<S> {
    pub fn pending(&self) -> PendingCounts {
        self.refs.counts()
    }

    pub fn pending_blocks(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.refs
            .outbox()
            .into_iter()
            .map(|(_, op)| op.block())
            .filter(|block| seen.insert(*block))
            .collect()
    }

    pub fn has_pending(&self, block: Uuid) -> bool {
        self.refs.has_pending(block)
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
        self.refs.push(Op::Move(Moving { block, parent }))?;
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
        let local = self.local_head(block);
        self.refs.remove(&settled)?;
        self.refs.set_remote(block, head)?;
        if local.is_none_or(|local| commits.contains(&local)) {
            self.refs.set_head(block, head)?;
        }
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
        let scope = match creation.parent {
            BlockParent::Block(parent) => {
                self.refs.summary(parent).and_then(|summary| summary.scope)
            }
            BlockParent::Root | BlockParent::Detached => Some(creation.block),
        };
        let summary = BlockSummary {
            id: creation.block,
            content_type: creation.content_type,
            access: Access::Manage,
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
        };
        self.refs.add_keys(&creation.keys)?;
        self.recorded(&creation.keys);
        let parent = creation.parent;
        self.refs.push(Op::Create(creation))?;
        self.refs
            .add_to_listing(Listing::Children(parent), &summary)?;
        self.note(&summary);
        Ok(summary)
    }

    pub(super) async fn read_contents_cached(
        &self,
        blocks: &[(Uuid, Option<CommitId>)],
        reason: String,
    ) -> Result<Vec<(BlockSummary, Option<Vec<u8>>)>, ClientError> {
        let mut read = Vec::with_capacity(blocks.len());
        for (block, _) in blocks {
            let mut summary = self
                .refs
                .summary(*block)
                .ok_or_else(|| ClientError::Disconnected(reason.clone()))?;
            summary.head = self.local_head(*block).or(summary.head);
            let content = match summary.head {
                Some(head) => Some(self.read_at(head).await?),
                None => None,
            };
            read.push((summary, content));
        }
        Ok(read)
    }

    pub(super) async fn read_at(&self, head: CommitId) -> Result<Vec<u8>, ClientError> {
        let commit = self.load_commit(head).await?;
        self.read_manifest(&commit.manifest).await
    }

    pub async fn drain(&self, skip: impl Fn(Uuid) -> bool) -> Result<Vec<Drained>, ClientError> {
        let mut outcomes: Vec<Drained> = Vec::new();
        let mut stopped = HashSet::new();
        for (sequence, op) in self.refs.outbox() {
            let block = op.block();
            if skip(block) || stopped.contains(&block) || !self.refs.holds(sequence) {
                continue;
            }
            let outcome = match op {
                Op::Create(creation) => self.send_creation(sequence, creation).await?,
                Op::Publish(pending) => self.drain_publish(sequence, pending).await?,
                Op::Move(moving) => self.drain_move(sequence, moving).await?,
                Op::Define(defining) => self.drain_define(sequence, defining).await?,
            };
            match outcome {
                Drained::Published { .. } => {
                    outcomes.retain(|held| {
                        !matches!(held, Drained::Published { block: other, .. } if *other == block)
                    });
                }
                Drained::Diverged { .. } | Drained::Unsent { .. } => {
                    stopped.insert(block);
                }
                Drained::Created { .. }
                | Drained::Moved { .. }
                | Drained::Defined { .. }
                | Drained::Refused { .. } => {}
            }
            outcomes.push(outcome);
        }
        Ok(outcomes)
    }

    pub async fn drain_block(&self, block: Uuid) -> Result<Vec<Drained>, ClientError> {
        self.drain(|other| other != block).await
    }

    async fn send_creation(
        &self,
        sequence: u64,
        creation: Creation,
    ) -> Result<Drained, ClientError> {
        let block = creation.block;
        let response = self
            .connection()
            .request(|request| ClientMessage::CreateBlock {
                request,
                block,
                content_type: creation.content_type,
                parent: creation.parent,
                metadata: creation.metadata.clone(),
                references: creation.references.clone(),
                key: creation.key,
                keys: creation.keys.clone(),
            })
            .await;
        match response {
            Ok(ServerMessage::Block { block: summary, .. }) => {
                self.refs.remove(&[sequence])?;
                self.note(&summary);
                Ok(Drained::Created { block })
            }
            Ok(_) => Err(ClientError::Unexpected),
            Err(ClientError::Refused(ErrorCode::BlockAlreadyExists, _)) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Created { block })
            }
            Err(error) if lost_access(&error) => self.lose_access(block),
            Err(error) => Err(error),
        }
    }

    async fn drain_move(&self, sequence: u64, moving: Moving) -> Result<Drained, ClientError> {
        let block = moving.block;
        match self.send_parent(block, moving.parent).await {
            Ok(()) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Moved { block })
            }
            Err(error) if lost_access(&error) => self.lose_access(block),
            Err(ClientError::Refused(code, _)) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Refused { block, code })
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
        let response = self
            .connection()
            .request(|request| ClientMessage::SetMetadata {
                request,
                block,
                metadata: defining.metadata.clone(),
                references: defining.references.clone(),
            })
            .await;
        match response {
            Ok(ServerMessage::Block { block: summary, .. }) => {
                self.refs.remove(&[sequence])?;
                self.note(&summary);
                Ok(Drained::Defined { block })
            }
            Ok(_) => Err(ClientError::Unexpected),
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
            Err(error) if lost_access(&error) => self.lose_access(block),
            Err(ClientError::Refused(code, _)) => {
                self.refs.remove(&[sequence])?;
                Ok(Drained::Refused { block, code })
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
                if self
                    .ancestors_among(head, &HashSet::from([pending.commit]))?
                    .contains(&pending.commit) =>
            {
                self.settle(sequence, &pending, head, false)?;
                Ok(Drained::Published { block, head })
            }
            Ok(Saved::Rejected { head }) => Ok(Drained::Diverged {
                block,
                local: self.local_head(block).unwrap_or(pending.commit),
                remote: head,
            }),
            Err(error) if lost_access(&error) => self.lose_access(block),
            Err(error) => Err(error),
        }
    }

    pub fn move_to_unsent(&self, block: Uuid) -> Result<bool, ClientError> {
        Ok(self.refs.move_to_unsent(block, now_milliseconds())?)
    }

    fn lose_access(&self, block: Uuid) -> Result<Drained, ClientError> {
        self.refs.lose_access(block)?;
        self.move_to_unsent(block)?;
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
            })
            .collect()
    }

    pub fn restore_unsent(&self, block: Uuid) -> Result<bool, ClientError> {
        let Some(unsent) = self.refs.take_unsent(block)? else {
            return Ok(false);
        };
        for op in unsent.ops {
            self.refs.push(op)?;
        }
        if let Some(head) = unsent.head {
            self.refs.set_head(block, head)?;
        }
        Ok(true)
    }

    pub fn discard_unsent(&self, block: Uuid) -> Result<bool, ClientError> {
        Ok(self.refs.take_unsent(block)?.is_some())
    }
}

pub(crate) fn unreachable(error: &ClientError) -> bool {
    error.is_unreachable()
}

pub(super) fn lost_access(error: &ClientError) -> bool {
    matches!(
        error,
        ClientError::Refused(ErrorCode::PermissionDenied | ErrorCode::BlockNotFound, _)
    )
}
