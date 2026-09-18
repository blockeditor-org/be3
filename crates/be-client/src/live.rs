use std::sync::Arc;

use be_block::{LiveEdit, Merge};
use be_commit::{CommitId, MergeResult};
use be_protocol::{ClientId, ServerMessage, SessionState};
use be_session::{Follower, Resume, Sequencer, SessionMessage, SessionOp, resume};
use be_store::ObjectStore;
use tokio::sync::broadcast::{Receiver, error::TryRecvError};
use uuid::Uuid;

use crate::{ClientError, Peer, Saved};

enum Role {
    Owner(Sequencer),
    Follower(Follower),
}

pub struct Live<S: ObjectStore, C: LiveEdit> {
    peer: Arc<Peer<S>>,
    block: Uuid,
    client: ClientId,
    state: SessionState,
    role: Role,
    confirmed: C,
    visible: C,
    base: Option<CommitId>,
    events: Receiver<ServerMessage>,
}

impl<S: ObjectStore, C: LiveEdit + Clone + Default> Live<S, C> {
    pub async fn join(peer: Arc<Peer<S>>, block: Uuid) -> Result<Self, ClientError> {
        let events = peer.connection().subscribe();
        let (client, state) = peer.join_session(block).await?;
        let head = peer.remote_head(block).await?;
        let confirmed = match head {
            Some(head) => peer.open_commit::<C>(head).await?,
            None => C::default(),
        };
        let role = if state.is_owner(client) {
            Role::Owner(Sequencer::new(head))
        } else {
            Role::Follower(Follower::new(client))
        };
        let live = Self {
            peer,
            block,
            client,
            state,
            role,
            visible: confirmed.clone(),
            confirmed,
            base: head,
            events,
        };
        if let Role::Follower(follower) = &live.role {
            let catchup = follower.catchup();
            live.send(live.state.owner, &catchup).await?;
        }
        Ok(live)
    }

    pub fn block(&self) -> Uuid {
        self.block
    }

    pub fn client(&self) -> ClientId {
        self.client
    }

    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn is_owner(&self) -> bool {
        matches!(self.role, Role::Owner(_))
    }

    pub fn content(&self) -> &C {
        &self.visible
    }

    pub fn head(&self) -> Option<CommitId> {
        self.base
    }

    pub fn set_head(&mut self, head: Option<CommitId>) {
        self.base = head;
    }

    pub fn is_clean(&self) -> bool {
        match &self.role {
            Role::Owner(sequencer) => sequencer.is_clean(),
            Role::Follower(follower) => follower.pending() == 0,
        }
    }

    pub async fn edit(&mut self, operation: C::Op) -> Result<(), ClientError> {
        self.visible.apply(&operation);
        let payload = C::encode_operation(&operation);
        match &mut self.role {
            Role::Owner(sequencer) => {
                let id = be_session::OpId {
                    client: self.client,
                    counter: sequencer.sequence() + 1,
                };
                let Some(op) = sequencer.accept(id, payload) else {
                    return Ok(());
                };
                self.confirmed.apply(&operation);
                let message = SessionMessage::Accepted { op };
                self.broadcast(&message).await
            }
            Role::Follower(follower) => {
                let message = follower.submit(payload);
                let owner = self.state.owner;
                self.send(owner, &message).await
            }
        }
    }

    pub async fn poll(&mut self) -> Result<usize, ClientError> {
        let mut handled = 0;
        loop {
            let event = match self.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
                Err(TryRecvError::Lagged(_)) => continue,
            };
            match event {
                ServerMessage::SessionChanged { block, state } if block == self.block => {
                    self.adopt(state);
                    handled += 1;
                }
                ServerMessage::Relayed {
                    block,
                    from,
                    payload,
                } if block == self.block => {
                    let plain = self.peer.unseal(&payload)?;
                    let Ok(message) = be_protocol::decode::<SessionMessage>(&plain) else {
                        continue;
                    };
                    self.receive(from, message).await?;
                    handled += 1;
                }
                _ => {}
            }
        }
        Ok(handled)
    }

    async fn receive(
        &mut self,
        from: ClientId,
        message: SessionMessage,
    ) -> Result<(), ClientError> {
        match message {
            SessionMessage::Submit { id, base, payload } => {
                let Role::Owner(sequencer) = &mut self.role else {
                    return Ok(());
                };
                let Ok(operation) = C::decode_operation(&payload) else {
                    return Ok(());
                };
                let onto: Vec<_> = sequencer
                    .since(base)
                    .iter()
                    .filter_map(|op| C::decode_operation(&op.payload).ok())
                    .collect();
                let Some(operation) = C::rebase(operation, &onto) else {
                    return Ok(());
                };
                let Some(op) = sequencer.accept(id, C::encode_operation(&operation)) else {
                    return Ok(());
                };
                self.confirmed.apply(&operation);
                self.visible = self.confirmed.clone();
                self.broadcast(&SessionMessage::Accepted { op }).await
            }
            SessionMessage::Accepted { op } => {
                self.apply_accepted(&op);
                Ok(())
            }
            SessionMessage::Catchup { since } => {
                let Role::Owner(sequencer) = &self.role else {
                    return Ok(());
                };
                let message = SessionMessage::Snapshot {
                    head: sequencer.head(),
                    sequence: sequencer.sequence(),
                    ops: sequencer.since(since),
                };
                self.send(Some(from), &message).await
            }
            SessionMessage::Snapshot {
                head,
                sequence,
                ops,
            } => {
                let Role::Follower(_) = &self.role else {
                    return Ok(());
                };
                if head != self.base
                    && let Some(head) = head
                {
                    self.confirmed = self.peer.open_commit::<C>(head).await?;
                    self.base = Some(head);
                    if let Role::Follower(follower) = &mut self.role {
                        follower.set_applied(sequence.saturating_sub(ops.len() as u64));
                    }
                }
                for op in &ops {
                    self.apply_accepted(op);
                }
                let Role::Follower(follower) = &mut self.role else {
                    return Ok(());
                };
                if ops.is_empty() {
                    let resubmit = follower.resynchronize(sequence);
                    let owner = self.state.owner;
                    for message in resubmit {
                        self.send(owner, &message).await?;
                    }
                }
                Ok(())
            }
        }
    }

    fn apply_accepted(&mut self, op: &SessionOp) {
        if let Role::Follower(follower) = &self.role
            && op.sequence <= follower.applied()
        {
            return;
        }
        let Ok(operation) = C::decode_operation(&op.payload) else {
            return;
        };
        self.confirmed.apply(&operation);
        let Role::Follower(follower) = &mut self.role else {
            self.visible = self.confirmed.clone();
            return;
        };
        if follower.is_mine(op.id) {
            follower.accepted(op);
        } else {
            follower.accepted(op);
            follower.rewrite_pending(|payload| {
                let pending = C::decode_operation(payload).ok()?;
                C::rebase(pending, std::slice::from_ref(&operation))
                    .map(|rebased| C::encode_operation(&rebased))
            });
        }
        let mut visible = self.confirmed.clone();
        for payload in follower.payloads() {
            if let Ok(pending) = C::decode_operation(&payload) {
                visible.apply(&pending);
            }
        }
        self.visible = visible;
    }

    fn adopt(&mut self, state: SessionState) {
        let became_owner = state.is_owner(self.client) && !self.is_owner();
        self.state = state;
        if became_owner {
            self.role = Role::Owner(Sequencer::new(self.base));
            self.confirmed = self.visible.clone();
        }
    }

    pub async fn claim(&mut self) -> Result<bool, ClientError> {
        let state = self
            .peer
            .claim_ownership(self.block, self.state.generation)
            .await?;
        let granted = state.is_owner(self.client);
        self.adopt(state);
        Ok(granted)
    }

    pub async fn seal(&mut self) -> Result<Saved, ClientError> {
        let Role::Owner(sequencer) = &mut self.role else {
            return Ok(Saved::Rejected { head: self.base });
        };
        let saved = self
            .peer
            .save(self.block, &self.confirmed, self.base)
            .await?;
        if let Some(head) = saved.published() {
            sequencer.sealed(head);
            self.base = Some(head);
            self.peer
                .heartbeat(self.block, self.state.generation, Some(head))
                .await?;
        }
        Ok(saved)
    }

    async fn broadcast(&self, message: &SessionMessage) -> Result<(), ClientError> {
        self.send(None, message).await
    }

    async fn send(
        &self,
        to: Option<ClientId>,
        message: &SessionMessage,
    ) -> Result<(), ClientError> {
        let payload = be_protocol::encode(message).map_err(|_| ClientError::Unexpected)?;
        self.peer.relay(self.block, to, &payload).await
    }
}

impl<S: ObjectStore, C: LiveEdit + Merge + Clone + Default> Live<S, C> {
    pub async fn reconcile(&mut self) -> Result<MergeResult<()>, ClientError> {
        let remote = self.peer.remote_head(self.block).await?;
        if let Some(remote) = remote {
            self.peer.fetch_history(remote).await?;
        }
        if let Some(local) = self.base {
            self.peer.fetch_history(local).await?;
        }
        match resume(self.peer.commits(), self.base, remote)? {
            Resume::Empty | Resume::UpToDate | Resume::Publish { .. } => Ok(MergeResult::Clean(())),
            Resume::FastForward { to } => {
                self.confirmed = self.peer.open_commit::<C>(to).await?;
                self.visible = self.confirmed.clone();
                self.base = Some(to);
                Ok(MergeResult::Clean(()))
            }
            Resume::Merge { base, ours, theirs } => {
                let ancestor = match base {
                    Some(base) => self.peer.open_commit::<C>(base).await?,
                    None => C::default(),
                };
                let ours_content = self.peer.open_commit::<C>(ours).await?;
                let theirs_content = self.peer.open_commit::<C>(theirs).await?;
                let merged = C::merge3(&ancestor, &ours_content, &theirs_content);
                let conflicts = match &merged {
                    MergeResult::Clean(_) => 0,
                    MergeResult::Conflicted { conflicts, .. } => *conflicts,
                };
                let value = match merged {
                    MergeResult::Clean(value) | MergeResult::Conflicted { value, .. } => value,
                };
                let saved = self
                    .peer
                    .save_merge(self.block, &value, Some(theirs), vec![ours])
                    .await?;
                if let Some(head) = saved.published() {
                    self.base = Some(head);
                }
                self.confirmed = value;
                self.visible = self.confirmed.clone();
                Ok(if conflicts == 0 {
                    MergeResult::Clean(())
                } else {
                    MergeResult::Conflicted {
                        value: (),
                        conflicts,
                    }
                })
            }
        }
    }
}
