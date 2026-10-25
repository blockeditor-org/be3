use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Instant,
};

use be_client::{ClientError, Peer, Saved, connection::Answer};
use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::{ClientId, ServerMessage, SessionState};
use block_type_api::ops::DeserializeOwned;
use block_type_api::{
    ByteRange, Change, Description, EditorId, Endpoint, Opening, PlainFile, PublishOutcome,
    Recipients, ReplicaId, Resume, Status,
    embed::{
        exports::replicas::{self, Replica},
        host::Call,
    },
    ops::{Connection, OpError, OpId, Outcome, Reply, Ticket},
};
use block_type_guest::editor::FromEditor;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use crate::{
    Host,
    modules::{self as types, Heard, Modules},
};

pub const HOST_EDITOR: EditorId = u64::MAX;

pub enum Answered {
    Read(Reply<Result<Vec<u8>, String>>, Result<Vec<u8>, String>),
    Publish(
        Reply<Result<PublishOutcome, String>>,
        Result<PublishOutcome, String>,
    ),
    Stash(Reply<Result<CommitId, String>>, Result<CommitId, String>),
    Resume(Reply<Result<Resume, String>>, Result<Resume, String>),
}

impl Answered {
    fn heads(&self) -> bool {
        matches!(self, Answered::Publish(..) | Answered::Stash(..))
    }

    fn send(self, connection: &mut Connection) {
        match self {
            Answered::Read(reply, result) => reply.send(connection, &result),
            Answered::Publish(reply, result) => reply.send(connection, &result),
            Answered::Stash(reply, result) => reply.send(connection, &result),
            Answered::Resume(reply, result) => reply.send(connection, &result),
        }
    }
}

pub enum Io {
    Answer {
        replica: ReplicaId,
        module: usize,
        generation: u64,
        epoch: u64,
        answer: Box<Answered>,
        published: Option<CommitId>,
    },
    Locked {
        replica: ReplicaId,
    },
    Relayed,
}

#[derive(Clone)]
enum Replayed {
    Message {
        from: Endpoint,
        bytes: Vec<u8>,
    },
    Moved {
        head: CommitId,
        from_this_device: bool,
    },
    Lease {
        session: SessionState,
        resynchronize: bool,
    },
}

impl Replayed {
    fn send(&self, replica: Replica, connection: &mut Connection) {
        match self.clone() {
            Replayed::Message { from, bytes } => {
                replica.message(connection, from, bytes);
            }
            Replayed::Moved {
                head,
                from_this_device,
            } => {
                replica.head_moved(connection, head, from_this_device);
            }
            Replayed::Lease {
                session,
                resynchronize,
            } => {
                replica.lease(connection, session, resynchronize);
            }
        }
    }
}

struct Open {
    block: Uuid,
    module: usize,
    handle: Replica,
    content_type: Uuid,
    client: ClientId,
    organization: Uuid,
    editors: BTreeMap<EditorId, bool>,
    status: Status,
    session: SessionState,
    reader: bool,
    publishing: usize,
    moved: Vec<CommitId>,
    published: Vec<CommitId>,
    replay: Vec<Replayed>,
    closing: bool,
    reopen: bool,
    epoch: u64,
}

pub struct Replicas<H: Host> {
    pub modules: Modules,
    answers: UnboundedReceiver<Io>,
    open: HashMap<ReplicaId, Open>,
    blocks: HashMap<Uuid, ReplicaId>,
    next: ReplicaId,
    io: UnboundedSender<Io>,
    handles: HashMap<(usize, OpId), ReplicaId>,
    contents: HashMap<(usize, OpId), ReplicaId>,
    replies: HashMap<(usize, OpId), tokio::sync::oneshot::Sender<Outcome>>,
    pending: Vec<Io>,
    relaying: usize,
    changed: bool,
    pub handled: u64,
    host: std::marker::PhantomData<fn() -> H>,
}

pub type Ctx<'a, H> = (&'a Arc<Peer<<H as Host>::Store>>, &'a Arc<Mutex<H>>);

impl<H: Host> Replicas<H> {
    pub fn new(modules: Modules) -> Self {
        let (io, answers) = unbounded_channel();
        Self {
            modules,
            answers,
            open: HashMap::new(),
            blocks: HashMap::new(),
            next: 0,
            io,
            handles: HashMap::new(),
            contents: HashMap::new(),
            replies: HashMap::new(),
            pending: Vec::new(),
            relaying: 0,
            changed: false,
            handled: 0,
            host: std::marker::PhantomData,
        }
    }

    pub fn relaying(&self) -> bool {
        self.relaying > 0
    }

    fn relay(&mut self, shared: &Arc<Mutex<H>>, sent: Result<Answer, ClientError>) {
        let answer = match sent {
            Ok(answer) => answer,
            Err(error) => return H::record(shared, error),
        };
        self.relaying += 1;
        let shared = Arc::clone(shared);
        let io = self.io.clone();
        H::spawn(async move {
            if let Err(error) = answer.await {
                H::record(&shared, error);
            }
            let _ = io.send(Io::Relayed);
            H::wake();
        });
    }

    pub async fn woken(&mut self) {
        let notified = std::pin::pin!(self.modules.notified());
        let answered = std::pin::pin!(self.answers.recv());
        if let futures_util::future::Either::Right((Some(io), _)) =
            futures_util::future::select(notified, answered).await
        {
            self.pending.push(io);
        }
    }

    pub async fn pump(&mut self, ctx: Ctx<'_, H>) -> bool {
        self.drain(ctx).await;
        std::mem::take(&mut self.changed)
    }

    async fn drain(&mut self, ctx: Ctx<'_, H>) {
        loop {
            while let Ok(io) = self.answers.try_recv() {
                self.pending.push(io);
            }
            let answers = std::mem::take(&mut self.pending);
            let heard = self.modules.take();
            for module in self.modules.take_restarted() {
                self.restart(module).await;
                self.changed = true;
            }
            if answers.is_empty() && heard.is_empty() {
                return;
            }
            self.handled += (answers.len() + heard.len()) as u64;
            for io in answers {
                self.answered(io, ctx.1);
            }
            for (module, heard) in heard {
                self.changed |= self.heard_from(ctx, module, heard).await;
            }
        }
    }

    pub async fn ask<T: DeserializeOwned>(
        &mut self,
        ctx: Ctx<'_, H>,
        module: usize,
        call: impl FnOnce(&mut Connection) -> Ticket<T>,
    ) -> Result<T, OpError> {
        let Some((ticket, answered)) = self.ask_later(module, call) else {
            return Err(OpError::Canceled);
        };
        let mut answered = std::pin::pin!(answered);
        loop {
            self.drain(ctx).await;
            if let Some(outcome) = futures_util::FutureExt::now_or_never(answered.as_mut()) {
                return match outcome {
                    Ok(outcome) => ticket.decode(outcome),
                    Err(_) => Err(OpError::Canceled),
                };
            }
            self.woken().await;
        }
    }

    fn ask_later<T: DeserializeOwned>(
        &mut self,
        module: usize,
        call: impl FnOnce(&mut Connection) -> Ticket<T>,
    ) -> Option<(Ticket<T>, tokio::sync::oneshot::Receiver<Outcome>)> {
        let ticket = self.modules.call(module, call)?;
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert((module, ticket.op()), answer);
        Some((ticket, answered))
    }

    pub fn is_open(&self, block: Uuid) -> bool {
        self.blocks.contains_key(&block)
    }

    pub fn open_blocks(&self) -> Vec<Uuid> {
        self.blocks.keys().copied().collect()
    }

    fn replica_of(&self, block: Uuid) -> Option<(ReplicaId, &Open)> {
        let replica = *self.blocks.get(&block)?;
        Some((replica, self.open.get(&replica)?))
    }

    pub async fn open(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
    ) {
        if let Some(replica) = self.blocks.get(&block).copied() {
            let Some(open) = self.open.get_mut(&replica) else {
                return;
            };
            open.closing = false;
            if reader && !open.reader {
                open.reader = true;
                self.attach_now(replica, HOST_EDITOR, true);
            }
            return;
        }
        let Some(kind) = types::kind_of(content_type) else {
            return;
        };
        let owned = !kind.frozen && H::owned(shared, block);
        let joined = async {
            peer.ensure_type(block, content_type, BlockParent::Detached)
                .await?;
            if owned && H::reshaped(shared, block, &kind.schema) {
                reset::<H>(peer, block, &kind).await?;
            }
            let (client, session) = peer.join_session(block).await?;
            peer.watch(block).await?;
            let head = peer.remote_head(block).await?;
            Ok::<_, ClientError>((client, session, head))
        }
        .await;
        let (client, session, head) = match joined {
            Ok(joined) => joined,
            Err(ClientError::Locked(_)) => {
                H::set_locked(shared, block);
                return;
            }
            Err(error) => {
                H::record(shared, error);
                return;
            }
        };
        if self.blocks.contains_key(&block) {
            return;
        }
        self.next += 1;
        let replica = self.next;
        let Some(module) = self.modules.module_of(content_type) else {
            return;
        };
        let organization = peer.organization();
        let tag = self.modules.tag(module);
        let opening = Opening {
            block,
            content_type,
            organization,
            client,
            tag,
            head,
            session: session.clone(),
        };
        let Some(handle) = self
            .modules
            .call(module, |connection| Replica::new(connection, opening))
        else {
            return;
        };
        self.blocks.insert(block, replica);
        self.handles.insert((module, handle.op()), replica);
        self.open.insert(
            replica,
            Open {
                block,
                module,
                handle,
                content_type,
                client,
                organization,
                editors: BTreeMap::new(),
                status: Status {
                    head,
                    ..Status::default()
                },
                session,
                reader,
                publishing: 0,
                moved: Vec::new(),
                published: Vec::new(),
                replay: Vec::new(),
                closing: false,
                reopen: false,
                epoch: 0,
            },
        );
        if reader {
            self.attach_now(replica, HOST_EDITOR, true);
        }
        let current = H::known_head(shared, block);
        if let Some(current) = current
            && Some(current) != head
        {
            self.moved(replica, current, false);
        }
    }

    fn attach_now(&mut self, replica: ReplicaId, editor: EditorId, editable: bool) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.editors.insert(editor, editable);
        let (module, handle) = (open.module, open.handle);
        self.modules.call(module, |connection| {
            handle.attach(connection, editor, editable);
        });
    }

    fn with_replica(&mut self, replica: ReplicaId, call: impl FnOnce(Replica, &mut Connection)) {
        if let Some(open) = self.open.get(&replica) {
            let (module, handle) = (open.module, open.handle);
            self.modules
                .call(module, |connection| call(handle, connection));
        }
    }

    fn replay(&mut self, replica: ReplicaId, replayed: Replayed) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.replay.push(replayed.clone());
        self.with_replica(replica, |handle, connection| {
            replayed.send(handle, connection);
        });
    }

    pub fn attach(&mut self, block: Uuid, editor: EditorId, editable: bool) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        if open.editors.get(&editor) == Some(&editable) {
            return;
        }
        self.attach_now(replica, editor, editable);
    }

    pub fn detach(&mut self, block: Uuid, editor: EditorId) -> bool {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return false;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return false;
        };
        if open.editors.remove(&editor).is_none() {
            return open.editors.is_empty();
        }
        let idle = open.editors.is_empty();
        self.with_replica(replica, |handle, connection| {
            handle.detach(connection, editor);
        });
        idle
    }

    pub fn relay_from_editor(
        &mut self,
        block: Uuid,
        editor: EditorId,
        bytes: Vec<u8>,
        at: Instant,
    ) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        self.modules.tick_at(open.module, at);
        self.replay(
            replica,
            Replayed::Message {
                from: Endpoint::Editor(editor),
                bytes,
            },
        );
    }

    pub fn send_as_host(&mut self, block: Uuid, message: &FromEditor, at: Instant) {
        if let Some((replica, open)) = self.replica_of(block)
            && !open.editors.contains_key(&HOST_EDITOR)
        {
            self.attach_now(replica, HOST_EDITOR, true);
        }
        self.relay_from_editor(block, HOST_EDITOR, message.encode(), at);
    }

    pub fn close(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if std::mem::replace(&mut open.closing, true) {
            return;
        }
        self.with_replica(replica, |handle, connection| {
            handle.resource_drop(connection)
        });
    }

    pub fn reopen(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.reopen = true;
        if std::mem::replace(&mut open.closing, true) {
            return;
        }
        self.with_replica(replica, |handle, connection| {
            handle.resource_drop(connection)
        });
    }

    pub fn moved_here(&mut self, block: Uuid, head: CommitId) {
        if let Some(replica) = self.blocks.get(&block).copied() {
            self.moved(replica, head, true);
        }
    }

    fn moved(&mut self, replica: ReplicaId, head: CommitId, from_this_device: bool) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if open.publishing > 0 && !from_this_device {
            open.moved.push(head);
            return;
        }
        if let Some(index) = open.published.iter().position(|held| *held == head) {
            open.published.drain(..=index);
            return;
        }
        self.replay(
            replica,
            Replayed::Moved {
                head,
                from_this_device,
            },
        );
    }

    async fn on_replica<T: DeserializeOwned>(
        &mut self,
        ctx: Ctx<'_, H>,
        block: Uuid,
        call: impl FnOnce(Replica, &mut Connection) -> Ticket<T>,
    ) -> Result<T, OpError> {
        let Some((_, open)) = self.replica_of(block) else {
            return Err(OpError::Canceled);
        };
        let (module, handle) = (open.module, open.handle);
        self.ask(ctx, module, |connection| call(handle, connection))
            .await
    }

    pub fn change_later(&mut self, block: Uuid, change: Change) {
        if let Some((replica, _)) = self.replica_of(block) {
            self.with_replica(replica, |handle, connection| {
                handle.change(connection, change);
            });
        }
    }

    pub async fn content(&mut self, ctx: Ctx<'_, H>, block: Uuid) -> Option<Vec<u8>> {
        self.on_replica(ctx, block, |handle, connection| handle.content(connection))
            .await
            .ok()
            .flatten()
    }

    pub async fn change(&mut self, ctx: Ctx<'_, H>, block: Uuid, change: Change) -> bool {
        self.on_replica(ctx, block, |handle, connection| {
            handle.change(connection, change)
        })
        .await
        .unwrap_or(false)
    }

    pub async fn seal(&mut self, ctx: Ctx<'_, H>, block: Uuid, force: bool) {
        let _ = self
            .on_replica(ctx, block, |handle, connection| {
                handle.seal(connection, force)
            })
            .await;
    }

    pub async fn seal_all(&mut self, ctx: Ctx<'_, H>) {
        for block in self.open_blocks() {
            self.seal(ctx, block, false).await;
        }
    }

    pub async fn merge(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        base: Option<&[u8]>,
        ours: Option<&[u8]>,
        theirs: Option<&[u8]>,
    ) -> Option<(Vec<u8>, bool)> {
        let module = self.modules.module_of(content_type)?;
        let (base, ours, theirs) = (
            base.map(<[u8]>::to_vec),
            ours.map(<[u8]>::to_vec),
            theirs.map(<[u8]>::to_vec),
        );
        match self
            .ask(ctx, module, |connection| {
                replicas::merge3(connection, content_type, base, ours, theirs)
            })
            .await
        {
            Ok(Ok(merged)) => Some((merged.bytes, merged.conflicts > 0)),
            _ => None,
        }
    }

    pub async fn describe(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: &[u8],
    ) -> Option<Description> {
        let module = self.modules.module_of(content_type)?;
        let (bytes, organization) = (bytes.to_vec(), ctx.0.organization());
        match self
            .ask(ctx, module, |connection| {
                replicas::describe(connection, content_type, bytes, organization)
            })
            .await
        {
            Ok(Ok(description)) => Some(description),
            _ => None,
        }
    }

    pub async fn to_file(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: Vec<u8>,
    ) -> Result<Option<PlainFile>, String> {
        let module = self
            .modules
            .module_of(content_type)
            .ok_or_else(|| format!("no type module has the block type {content_type}"))?;
        match self
            .ask(ctx, module, |connection| {
                replicas::to_file(connection, content_type, bytes)
            })
            .await
        {
            Ok(Ok(file)) => Ok(file),
            Ok(Err(block_type_api::Failure::Malformed(error))) => Err(error),
            Ok(Err(block_type_api::Failure::UnknownType)) => {
                Err(format!("no type module has the block type {content_type}"))
            }
            Err(error) => Err(format!("the type module answered: {error}")),
        }
    }

    pub async fn apply(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> Result<Option<Vec<u8>>, String> {
        let module = self
            .modules
            .module_of(content_type)
            .ok_or_else(|| format!("no type module has the block type {content_type}"))?;
        match self
            .ask(ctx, module, |connection| {
                replicas::apply(connection, content_type, bytes, client, change)
            })
            .await
        {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(block_type_api::Failure::Malformed(error))) => Err(error),
            Ok(Err(block_type_api::Failure::UnknownType)) => {
                Err(format!("no type module has the block type {content_type}"))
            }
            Err(error) => Err(format!("the type module answered: {error}")),
        }
    }

    pub fn status(&self, block: Uuid) -> Option<Status> {
        self.replica_of(block).map(|(_, open)| open.status.clone())
    }

    pub fn session(&self, block: Uuid) -> Option<(ClientId, &SessionState)> {
        self.replica_of(block)
            .map(|(_, open)| (open.client, &open.session))
    }

    pub fn claim(&mut self, block: Uuid) {
        if let Some((replica, _)) = self.replica_of(block) {
            self.with_replica(replica, |handle, connection| handle.claim(connection));
        }
    }

    pub async fn heard(&mut self, (peer, shared): Ctx<'_, H>, event: &ServerMessage) {
        match event {
            ServerMessage::SessionChanged { block, state } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                open.session = state.clone();
                self.replay(
                    replica,
                    Replayed::Lease {
                        session: state.clone(),
                        resynchronize: false,
                    },
                );
            }
            ServerMessage::Relayed {
                block,
                from,
                payload,
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let bytes = match peer.unseal(payload).await {
                    Ok(bytes) => bytes,
                    Err(error) => return H::record(shared, error),
                };
                self.replay(
                    replica,
                    Replayed::Message {
                        from: Endpoint::Peer(*from),
                        bytes,
                    },
                );
            }
            ServerMessage::HeadChanged { block, head, .. } => {
                if let Some(replica) = self.blocks.get(block).copied() {
                    self.moved(replica, *head, false);
                }
            }
            _ => {}
        }
    }

    pub async fn resynchronize(&mut self, (peer, shared): Ctx<'_, H>) {
        for (replica, block) in self
            .open
            .iter()
            .map(|(replica, open)| (*replica, open.block))
            .collect::<Vec<_>>()
        {
            let session = match peer.join_session(block).await {
                Ok((_, session)) => session,
                Err(error) => {
                    H::record(shared, error);
                    continue;
                }
            };
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            open.session = session.clone();
            self.with_replica(replica, |handle, connection| {
                handle.lease(connection, session, true);
            });
        }
    }

    pub fn answered(&mut self, io: Io, shared: &Arc<Mutex<H>>) {
        match io {
            Io::Answer {
                replica,
                module,
                generation,
                epoch,
                answer,
                published,
            } => {
                let heads = answer.heads();
                if self.modules.generation(module) == generation {
                    self.modules
                        .call(module, |connection| answer.send(connection));
                }
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.epoch != epoch {
                    return;
                }
                if heads {
                    open.publishing = open.publishing.saturating_sub(1);
                }
                if let Some(head) = published {
                    open.published.push(head);
                }
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.publishing == 0 {
                    for head in std::mem::take(&mut open.moved) {
                        self.moved(replica, head, false);
                    }
                }
            }
            Io::Locked { replica } => {
                if let Some(open) = self.open.get(&replica) {
                    H::set_locked(shared, open.block);
                }
            }
            Io::Relayed => self.relaying -= 1,
        }
    }

    pub async fn reconnect(&mut self, (peer, shared): Ctx<'_, H>) {
        let replicas: Vec<ReplicaId> = self.open.keys().copied().collect();
        for replica in replicas {
            let Some(block) = self.open.get(&replica).map(|open| open.block) else {
                continue;
            };
            let joined = async {
                let (client, session) = peer.join_session(block).await?;
                peer.watch(block).await?;
                let head = peer.remote_head(block).await?;
                Ok::<_, ClientError>((client, session, head))
            }
            .await;
            let (client, session, head) = match joined {
                Ok(joined) => joined,
                Err(error) => {
                    H::record(shared, error);
                    continue;
                }
            };
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            open.client = client;
            open.session = session.clone();
            open.epoch += 1;
            open.publishing = 0;
            open.moved.clear();
            open.published.clear();
            open.replay.retain(|replayed| {
                matches!(
                    replayed,
                    Replayed::Message {
                        from: Endpoint::Editor(_),
                        ..
                    }
                )
            });
            let opening = Opening {
                block,
                content_type: open.content_type,
                organization: open.organization,
                client,
                tag: self.modules.tag(open.module),
                head: open.status.head,
                session,
            };
            let editors: Vec<(EditorId, bool)> = open
                .editors
                .iter()
                .map(|(editor, editable)| (*editor, *editable))
                .collect();
            let replay = open.replay.clone();
            let known = open.status.head;
            self.with_replica(replica, |handle, connection| {
                handle.reopen(connection, opening);
                for (editor, editable) in editors {
                    handle.attach(connection, editor, editable);
                }
                for replayed in &replay {
                    replayed.send(handle, connection);
                }
            });
            if let Some(head) = head
                && Some(head) != known
            {
                self.moved(replica, head, false);
            }
        }
    }

    pub async fn restart(&mut self, module: usize) {
        let restarted: Vec<ReplicaId> = self
            .open
            .iter()
            .filter(|(_, open)| open.module == module)
            .map(|(replica, _)| *replica)
            .collect();
        self.handles.retain(|(from, _), _| *from != module);
        self.contents.retain(|(from, _), _| *from != module);
        self.replies.retain(|(from, _), _| *from != module);
        for replica in restarted {
            let tag = self.modules.tag(module);
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            open.epoch += 1;
            open.publishing = 0;
            open.moved.clear();
            let opening = Opening {
                block: open.block,
                content_type: open.content_type,
                organization: open.organization,
                client: open.client,
                tag,
                head: open.status.head,
                session: open.session.clone(),
            };
            let editors: Vec<(EditorId, bool)> = open
                .editors
                .iter()
                .map(|(editor, editable)| (*editor, *editable))
                .collect();
            let replay = open.replay.clone();
            let closing = open.closing;
            let Some(handle) = self.modules.call(module, |connection| {
                let handle = Replica::new(connection, opening);
                for (editor, editable) in editors {
                    handle.attach(connection, editor, editable);
                }
                for replayed in &replay {
                    replayed.send(handle, connection);
                }
                if closing {
                    handle.resource_drop(connection);
                }
                handle
            }) else {
                continue;
            };
            if let Some(open) = self.open.get_mut(&replica) {
                open.handle = handle;
            }
            self.handles.insert((module, handle.op()), replica);
        }
    }

    async fn heard_from(&mut self, ctx: Ctx<'_, H>, module: usize, heard: Heard) -> bool {
        match heard {
            Heard::Hello(_) => false,
            Heard::Canceled { op } => {
                self.refuse(module, op);
                false
            }
            Heard::Completed { op, outcome } => self.completed(ctx, module, op, outcome).await,
            Heard::Call { op, parent, call } => {
                let replica = parent
                    .and_then(|parent| self.handles.get(&(module, parent)).copied())
                    .filter(|replica| self.open.contains_key(replica));
                match replica {
                    Some(replica) => self.act(ctx, module, replica, call).await,
                    None => {
                        if let Call::Error { message } = call {
                            H::report(ctx.1, format!("a type module: {message}"));
                        }
                        if let Some(op) = op {
                            self.refuse(module, op);
                        }
                        false
                    }
                }
            }
        }
    }

    fn refuse(&mut self, module: usize, op: OpId) {
        self.modules.call(module, |connection| {
            connection.complete(op, Outcome::Err(OpError::Canceled));
        });
    }

    async fn completed(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        op: OpId,
        outcome: Outcome,
    ) -> bool {
        if let Some(answer) = self.replies.remove(&(module, op)) {
            let _ = answer.send(outcome);
            return false;
        }
        if let Some(replica) = self.contents.remove(&(module, op)) {
            let (Some(open), Ok(Some(bytes))) =
                (self.open.get(&replica), outcome.decode::<Option<Vec<u8>>>())
            else {
                return false;
            };
            H::content(shared, open.block, open.content_type, bytes);
            return true;
        }
        let Some(replica) = self.handles.remove(&(module, op)) else {
            return false;
        };
        let Some(open) = self.open.remove(&replica) else {
            return false;
        };
        self.blocks.remove(&open.block);
        let _ = peer.leave_session(open.block).await;
        let _ = peer.unwatch(open.block).await;
        if open.reopen {
            self.open((peer, shared), open.block, open.content_type, open.reader)
                .await;
            return true;
        }
        H::released(shared, open.block);
        self.counted(shared);
        true
    }

    fn answer_later(
        &self,
        replica: ReplicaId,
        module: usize,
        task: impl Future<Output = (Answered, Option<CommitId>)> + 'static,
    ) {
        let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
        let generation = self.modules.generation(module);
        let io = self.io.clone();
        H::spawn(async move {
            let (answer, published) = task.await;
            let _ = io.send(Io::Answer {
                replica,
                module,
                generation,
                epoch,
                answer: Box::new(answer),
                published,
            });
            H::wake();
        });
    }

    async fn act(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        replica: ReplicaId,
        call: Call,
    ) -> bool {
        let Some((block, content_type)) = self
            .open
            .get(&replica)
            .map(|open| (open.block, open.content_type))
        else {
            return false;
        };
        match call {
            Call::Clock { .. } | Call::SleepUntil { .. } => false,
            Call::Send { to, bytes } => match to {
                Recipients::One(Endpoint::Editor(editor)) => {
                    H::to_editor(shared, block, editor, bytes)
                }
                Recipients::One(Endpoint::Host) => false,
                Recipients::One(Endpoint::Peer(client)) => {
                    self.relay(shared, peer.relay(block, Some(client), &bytes).await);
                    false
                }
                Recipients::Peers => {
                    self.relay(shared, peer.relay(block, None, &bytes).await);
                    false
                }
            },
            Call::Read {
                reply,
                commit,
                range,
            } => {
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                self.answer_later(replica, module, async move {
                    let read = async {
                        let commit = peer.load_commit(commit).await?;
                        peer.read_manifest(&commit.manifest).await
                    }
                    .await;
                    if let Err(ClientError::Locked(_)) = &read {
                        let _ = io.send(Io::Locked { replica });
                    }
                    let result = read
                        .map(|bytes| slice(bytes, range))
                        .map_err(|error| error.to_string());
                    (Answered::Read(reply, result), None)
                });
                false
            }
            Call::Publish {
                reply,
                expected,
                merging,
                content,
            } => {
                if let Some(open) = self.open.get_mut(&replica) {
                    open.publishing += 1;
                }
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let saved = peer
                        .publish_encoded(
                            block,
                            content_type,
                            &content.bytes,
                            content.references,
                            content.derived,
                            expected,
                            merging.map(|merging| (merging.base, merging.ours)),
                        )
                        .await;
                    let (result, published) = match saved {
                        Ok(Saved::Published(head)) => {
                            (Ok(PublishOutcome::Published(head)), Some(head))
                        }
                        Ok(Saved::Unchanged(head)) => (Ok(PublishOutcome::Published(head)), None),
                        Ok(Saved::Rejected { head }) => (Ok(PublishOutcome::Rejected(head)), None),
                        Err(error) => (Err(error.to_string()), None),
                    };
                    (Answered::Publish(reply, result), published)
                });
                false
            }
            Call::Stash {
                reply,
                parent,
                content,
            } => {
                if let Some(open) = self.open.get_mut(&replica) {
                    open.publishing += 1;
                }
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let stashed = peer
                        .stash_encoded(
                            block,
                            content_type,
                            &content.bytes,
                            content.references,
                            content.derived,
                            parent,
                        )
                        .await
                        .map_err(|error| error.to_string());
                    (Answered::Stash(reply, stashed), None)
                });
                false
            }
            Call::ResumeSession { reply, local } => {
                let peer = Arc::clone(peer);
                self.answer_later(replica, module, async move {
                    let resumed = async {
                        let remote = peer.remote_head(block).await?;
                        if let Some(remote) = remote {
                            peer.fetch_history(remote).await?;
                        }
                        if let Some(local) = local {
                            peer.fetch_history(local).await?;
                        }
                        Ok::<_, ClientError>(be_session::resume(peer.commits(), local, remote)?)
                    }
                    .await
                    .map_err(|error| error.to_string());
                    (Answered::Resume(reply, resumed), None)
                });
                false
            }
            Call::ClaimLease { generation } => {
                match peer.claim_ownership(block, generation).await {
                    Ok(state) => {
                        if let Some(open) = self.open.get_mut(&replica) {
                            open.session = state.clone();
                        }
                        self.replay(
                            replica,
                            Replayed::Lease {
                                session: state,
                                resynchronize: false,
                            },
                        );
                    }
                    Err(error) => H::record(shared, error),
                }
                false
            }
            Call::ReportClean {
                generation,
                clean_at,
            } => {
                if let Err(error) = peer.heartbeat(block, generation, clean_at).await {
                    H::record(shared, error);
                }
                false
            }
            Call::ReportStatus { status } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                if status.clean && status.head.is_some() && !status.owes_seal {
                    open.replay.clear();
                }
                let became_unreadable =
                    status.unreadable.is_some() && open.status.unreadable.is_none();
                open.status = status.clone();
                if became_unreadable {
                    let owned = H::owned(shared, block);
                    if let Some(kind) = types::kind_of(content_type)
                        && owned
                        && !kind.frozen
                    {
                        match reset::<H>(peer, block, &kind).await {
                            Ok(()) => {
                                if let Some(open) = self.open.get_mut(&replica) {
                                    open.reopen = true;
                                    open.closing = true;
                                }
                                self.with_replica(replica, |handle, connection| {
                                    handle.resource_drop(connection);
                                });
                                return true;
                            }
                            Err(error) => H::record(shared, error),
                        }
                    }
                }
                H::status(shared, block, &status);
                self.counted(shared);
                true
            }
            Call::Described { description } => {
                let reader = self.open.get(&replica).is_some_and(|open| open.reader);
                let references = peer.resolve(block, description.references.clone());
                H::described(shared, block, references, description);
                if reader && let Some(open) = self.open.get(&replica) {
                    let handle = open.handle;
                    if let Some(ticket) = self
                        .modules
                        .call(module, |connection| handle.content(connection))
                    {
                        self.contents.insert((module, ticket.op()), replica);
                    }
                }
                true
            }
            Call::History { can_undo, can_redo } => {
                H::history(shared, block, can_undo, can_redo);
                true
            }
            Call::Conflict {
                ours,
                theirs,
                count,
            } => {
                H::conflict(
                    shared,
                    block,
                    crate::Conflict {
                        ours,
                        theirs,
                        count: count as usize,
                    },
                );
                true
            }
            Call::Error { message } => {
                H::report(shared, format!("{block}: {message}"));
                false
            }
        }
    }

    fn counted(&self, shared: &Arc<Mutex<H>>) {
        let unsealed = self.open.values().filter(|open| !open.status.clean).count();
        let owing = self
            .open
            .values()
            .filter(|open| open.status.owes_seal)
            .count();
        H::counted(shared, unsealed, owing);
    }
}

fn slice(bytes: Vec<u8>, range: Option<ByteRange>) -> Vec<u8> {
    let Some(range) = range else {
        return bytes;
    };
    let start = (range.offset as usize).min(bytes.len());
    let end = start.saturating_add(range.length as usize).min(bytes.len());
    bytes[start..end].to_vec()
}

async fn reset<H: Host>(
    peer: &Peer<H::Store>,
    block: Uuid,
    kind: &types::Kind,
) -> Result<(), ClientError> {
    let mut expected = peer.remote_head(block).await?;
    for _ in 0..4 {
        match peer
            .publish_encoded(
                block,
                kind.content_type,
                &kind.blank,
                kind.described.references.clone(),
                kind.described.derived.clone(),
                expected,
                None,
            )
            .await?
        {
            Saved::Rejected { head } => expected = head,
            _ => break,
        }
    }
    H::reset_notice();
    Ok(())
}
