use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use be_block::{LiveEdit, Merge};
use be_commit::{CommitId, MergeResult};
use be_protocol::{ClientId, ServerMessage, SessionState};
use be_session::{Follower, Resume, Sequencer, SessionMessage, SessionOp, resume};
use be_store::ObjectStore;
use tokio::sync::broadcast::{
    Receiver,
    error::{RecvError, TryRecvError},
};
use uuid::Uuid;

use crate::{ClientError, Peer, Saved};

const JOURNAL_LIMIT: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Journaled<Op> {
    Edited(Op),
    Applied(Op),
    Replaced,
}

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
    sealed: u64,
    reload: bool,
    events: Receiver<ServerMessage>,
    journal: Vec<Journaled<C::Op>>,
    presence: BTreeMap<(ClientId, Uuid), Vec<u8>>,
    shown: BTreeMap<Uuid, Vec<u8>>,
    presence_changed: bool,
    moved: bool,
}

impl<S: ObjectStore, C: LiveEdit + Clone + Default> Live<S, C> {
    pub async fn join(peer: Arc<Peer<S>>, block: Uuid) -> Result<Self, ClientError> {
        let events = peer.connection().subscribe();
        let (client, state) = peer.join_session(block).await?;
        peer.watch(block).await?;
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
            sealed: 0,
            reload: false,
            events,
            journal: Vec::new(),
            presence: BTreeMap::new(),
            shown: BTreeMap::new(),
            presence_changed: false,
            moved: false,
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

    pub fn take_journal(&mut self) -> Vec<Journaled<C::Op>> {
        std::mem::take(&mut self.journal)
    }

    pub fn presence(&self) -> &BTreeMap<(ClientId, Uuid), Vec<u8>> {
        &self.presence
    }

    pub fn take_presence_changed(&mut self) -> bool {
        std::mem::take(&mut self.presence_changed)
    }

    pub async fn set_presence(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> Result<(), ClientError> {
        let unchanged = match &value {
            Some(value) => self.shown.get(&kind) == Some(value),
            None => !self.shown.contains_key(&kind),
        };
        if unchanged {
            return Ok(());
        }
        match &value {
            Some(value) => self.shown.insert(kind, value.clone()),
            None => self.shown.remove(&kind),
        };
        self.broadcast(&SessionMessage::Presence { kind, value })
            .await
    }

    async fn show_presence_to(&self, client: ClientId) -> Result<(), ClientError> {
        for (kind, value) in &self.shown {
            let message = SessionMessage::Presence {
                kind: *kind,
                value: Some(value.clone()),
            };
            self.send(Some(client), &message).await?;
        }
        Ok(())
    }

    fn journal(&mut self, entry: Journaled<C::Op>) {
        if self.journal.len() >= JOURNAL_LIMIT {
            self.journal.clear();
            self.journal.push(Journaled::Replaced);
        }
        if matches!(entry, Journaled::Replaced) {
            self.journal.clear();
        }
        self.journal.push(entry);
    }

    fn replace_visible(&mut self, visible: C) {
        self.visible = visible;
        self.journal(Journaled::Replaced);
    }

    pub async fn edit(&mut self, operation: C::Op) -> Result<(), ClientError> {
        self.visible.apply(&operation);
        self.journal(Journaled::Edited(operation.clone()));
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
            handled += self.handle(event).await?;
        }
        Ok(handled)
    }

    pub async fn wait(&mut self) -> Result<usize, ClientError> {
        let event = loop {
            match self.events.recv().await {
                Ok(event) => break event,
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => {
                    return Err(ClientError::Disconnected(
                        "the connection closed while waiting on the session".into(),
                    ));
                }
            }
        };
        let handled = self.handle(event).await?;
        Ok(handled + self.poll().await?)
    }

    async fn handle(&mut self, event: ServerMessage) -> Result<usize, ClientError> {
        match event {
            ServerMessage::SessionChanged { block, state } if block == self.block => {
                self.adopt(state).await?;
                Ok(1)
            }
            ServerMessage::HeadChanged { block, head, .. } if block == self.block => {
                if self.is_owner() && self.base != Some(head) {
                    self.moved = true;
                }
                Ok(0)
            }
            ServerMessage::Relayed {
                block,
                from,
                payload,
            } if block == self.block => {
                let plain = self.peer.unseal(&payload)?;
                let Ok(message) = be_protocol::decode::<SessionMessage>(&plain) else {
                    return Ok(0);
                };
                self.receive(from, message).await?;
                Ok(1)
            }
            _ => Ok(0),
        }
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
                self.visible.apply(&operation);
                self.journal(Journaled::Applied(operation));
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
                if sequencer.inherited() > 0 {
                    self.reload = true;
                }
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
                let sealed = sequence.saturating_sub(ops.len() as u64);
                if head != self.base
                    && let Some(head) = head
                {
                    self.confirmed = self.peer.open_commit::<C>(head).await?;
                    self.base = Some(head);
                    if let Role::Follower(follower) = &mut self.role {
                        follower.set_applied(sealed);
                    }
                }
                self.sealed = sealed;
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
            SessionMessage::Replaced { head } => self.adopt_replacement(head).await,
            SessionMessage::Presence { kind, value } => {
                if from == self.client {
                    return Ok(());
                }
                let changed = match value {
                    Some(value) => self.presence.insert((from, kind), value.clone()) != Some(value),
                    None => self.presence.remove(&(from, kind)).is_some(),
                };
                self.presence_changed |= changed;
                Ok(())
            }
            SessionMessage::Sealed {
                head,
                sequence,
                reload,
            } => {
                let Role::Follower(follower) = &self.role else {
                    return Ok(());
                };
                if !reload && follower.applied() != sequence {
                    let catchup = follower.catchup();
                    let owner = self.state.owner;
                    return self.send(owner, &catchup).await;
                }
                if reload {
                    self.confirmed = self.peer.open_commit::<C>(head).await?;
                    if let Role::Follower(follower) = &mut self.role {
                        follower.set_applied(sequence);
                    }
                    self.rebuild_visible();
                }
                self.base = Some(head);
                self.sealed = sequence;
                Ok(())
            }
        }
    }

    fn rebuild_visible(&mut self) {
        let mut visible = self.confirmed.clone();
        if let Role::Follower(follower) = &self.role {
            for payload in follower.payloads() {
                if let Ok(pending) = C::decode_operation(&payload) {
                    visible.apply(&pending);
                }
            }
        }
        self.replace_visible(visible);
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
            self.visible.apply(&operation);
            self.journal(Journaled::Applied(operation));
            return;
        };
        let unchanged = follower.is_mine(op.id)
            && follower.front() == Some(op.id)
            && follower
                .payloads()
                .first()
                .is_some_and(|payload| *payload == op.payload);
        if unchanged {
            follower.accepted(op);
            return;
        }
        if !follower.is_mine(op.id) && follower.pending() == 0 {
            follower.accepted(op);
            self.visible.apply(&operation);
            self.journal(Journaled::Applied(operation));
            return;
        }
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
        self.rebuild_visible();
    }

    async fn adopt(&mut self, state: SessionState) -> Result<(), ClientError> {
        let owner_changed = state.owner != self.state.owner;
        let known: BTreeSet<ClientId> = self.state.participants.iter().copied().collect();
        let joined: Vec<ClientId> = state
            .participants
            .iter()
            .copied()
            .filter(|client| *client != self.client && !known.contains(client))
            .collect();
        let before = self.presence.len();
        self.presence
            .retain(|(client, _), _| state.participants.contains(client));
        self.presence_changed |= self.presence.len() != before;
        self.state = state;
        for client in joined {
            self.show_presence_to(client).await?;
        }
        let Role::Follower(follower) = &mut self.role else {
            return Ok(());
        };
        if !owner_changed {
            return Ok(());
        }
        if !self.state.is_owner(self.client) {
            let applied = follower.applied();
            let resubmit = follower.resynchronize(applied);
            let owner = self.state.owner;
            for message in resubmit {
                self.send(owner, &message).await?;
            }
            return Ok(());
        }
        let applied = follower.applied();
        let pending = follower.take_pending();
        let mut sequencer =
            Sequencer::continuing(self.base, applied, applied.saturating_sub(self.sealed));
        let mut accepted = Vec::new();
        for (id, payload) in pending {
            let Ok(operation) = C::decode_operation(&payload) else {
                continue;
            };
            if let Some(op) = sequencer.accept(id, payload) {
                self.confirmed.apply(&operation);
                accepted.push(op);
            }
        }
        self.role = Role::Owner(sequencer);
        self.replace_visible(self.confirmed.clone());
        for op in accepted {
            self.broadcast(&SessionMessage::Accepted { op }).await?;
        }
        Ok(())
    }

    pub async fn claim(&mut self) -> Result<bool, ClientError> {
        let state = self
            .peer
            .claim_ownership(self.block, self.state.generation)
            .await?;
        let granted = state.is_owner(self.client);
        self.adopt(state).await?;
        Ok(granted)
    }

    pub async fn replace(&mut self, content: C) -> Result<bool, ClientError> {
        let mut expected = self.base;
        for _ in 0..4 {
            match self.peer.save(self.block, &content, expected).await? {
                Saved::Published(head) | Saved::Unchanged(head) => {
                    if self.is_owner() {
                        self.confirmed = content;
                        self.replace_visible(self.confirmed.clone());
                        self.reload = true;
                        self.settle_at(head).await?;
                    } else {
                        self.replace_visible(content);
                        let owner = self.state.owner;
                        self.send(owner, &SessionMessage::Replaced { head }).await?;
                    }
                    return Ok(true);
                }
                Saved::Rejected { head } => expected = head,
            }
        }
        Ok(false)
    }

    async fn adopt_replacement(&mut self, head: CommitId) -> Result<(), ClientError> {
        let Role::Owner(sequencer) = &self.role else {
            return Ok(());
        };
        if self.base == Some(head) {
            return Ok(());
        }
        let unsealed: Vec<C::Op> = sequencer
            .since(0)
            .iter()
            .filter_map(|op| C::decode_operation(&op.payload).ok())
            .collect();
        let mut replaced = self.peer.open_commit::<C>(head).await?;
        for operation in &unsealed {
            replaced.apply(operation);
        }
        self.confirmed = replaced;
        self.replace_visible(self.confirmed.clone());
        self.base = Some(head);
        self.reload = true;
        if unsealed.is_empty() {
            self.settle_at(head).await
        } else {
            self.seal().await.map(|_| ())
        }
    }

    pub async fn seal(&mut self) -> Result<Saved, ClientError> {
        if !self.is_owner() {
            return Ok(Saved::Rejected { head: self.base });
        }
        let saved = self
            .peer
            .save(self.block, &self.confirmed, self.base)
            .await?;
        if let Some(head) = saved.published() {
            self.settle_at(head).await?;
        }
        Ok(saved)
    }

    async fn settle_at(&mut self, head: CommitId) -> Result<(), ClientError> {
        self.base = Some(head);
        let Role::Owner(sequencer) = &mut self.role else {
            return Ok(());
        };
        sequencer.sealed(head);
        let sequence = sequencer.sequence();
        self.sealed = sequence;
        let reload = std::mem::take(&mut self.reload);
        self.broadcast(&SessionMessage::Sealed {
            head,
            sequence,
            reload,
        })
        .await?;
        self.peer
            .heartbeat(self.block, self.state.generation, Some(head))
            .await?;
        Ok(())
    }

    fn has_unsealed_work(&self) -> bool {
        matches!(&self.role, Role::Owner(sequencer) if !sequencer.is_clean())
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
    pub async fn catch_up(&mut self) -> Result<(), ClientError> {
        if std::mem::take(&mut self.moved) {
            self.reconcile().await?;
        }
        Ok(())
    }

    pub async fn published_elsewhere(&mut self, head: CommitId) -> Result<(), ClientError> {
        if self.base == Some(head) {
            return Ok(());
        }
        if self.is_owner() {
            self.moved = true;
            return self.catch_up().await;
        }
        let owner = self.state.owner;
        self.send(owner, &SessionMessage::Replaced { head }).await
    }

    pub async fn reconcile(&mut self) -> Result<MergeResult<()>, ClientError> {
        let remote = self.peer.remote_head(self.block).await?;
        if let Some(remote) = remote {
            self.peer.fetch_history(remote).await?;
        }
        if let Some(local) = self.base {
            self.peer.fetch_history(local).await?;
        }
        let unsealed = self.has_unsealed_work();
        match resume(self.peer.commits(), self.base, remote)? {
            Resume::Empty | Resume::UpToDate | Resume::Publish { .. } => Ok(MergeResult::Clean(())),
            Resume::FastForward { to } if !unsealed => {
                self.confirmed = self.peer.open_commit::<C>(to).await?;
                self.replace_visible(self.confirmed.clone());
                self.reload = true;
                self.settle_at(to).await?;
                Ok(MergeResult::Clean(()))
            }
            Resume::FastForward { to } => {
                let ancestor = self.open_or_default(self.base).await?;
                let theirs = self.peer.open_commit::<C>(to).await?;
                let merged = C::merge3(&ancestor, &self.confirmed, &theirs);
                let (value, outcome) = split(merged);
                let saved = self.peer.save(self.block, &value, Some(to)).await?;
                self.take_merged(value, saved.published()).await?;
                Ok(outcome)
            }
            Resume::Merge { base, ours, theirs } => {
                let ancestor = self.open_or_default(base).await?;
                let ours_content = match unsealed {
                    true => self.confirmed.clone(),
                    false => self.peer.open_commit::<C>(ours).await?,
                };
                let theirs_content = self.peer.open_commit::<C>(theirs).await?;
                let merged = C::merge3(&ancestor, &ours_content, &theirs_content);
                let (value, outcome) = split(merged);
                let saved = self
                    .peer
                    .save_merge(self.block, &value, Some(theirs), vec![ours])
                    .await?;
                self.take_merged(value, saved.published()).await?;
                Ok(outcome)
            }
        }
    }

    async fn open_or_default(&self, commit: Option<CommitId>) -> Result<C, ClientError> {
        match commit {
            Some(commit) => self.peer.open_commit::<C>(commit).await,
            None => Ok(C::default()),
        }
    }

    async fn take_merged(
        &mut self,
        value: C,
        published: Option<CommitId>,
    ) -> Result<(), ClientError> {
        self.confirmed = value;
        self.replace_visible(self.confirmed.clone());
        self.reload = true;
        match published {
            Some(head) => self.settle_at(head).await,
            None => {
                if let Role::Owner(sequencer) = &mut self.role {
                    sequencer.carry_unsealed();
                }
                Ok(())
            }
        }
    }
}

fn split<C>(merged: MergeResult<C>) -> (C, MergeResult<()>) {
    match merged {
        MergeResult::Clean(value) => (value, MergeResult::Clean(())),
        MergeResult::Conflicted { value, conflicts } => (
            value,
            MergeResult::Conflicted {
                value: (),
                conflicts,
            },
        ),
    }
}
