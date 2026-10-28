use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::Arc,
    time::Instant,
};

use std::sync::PoisonError;
use wasi_threads::Mutex;

use be_client::{ClientError, Peer, Reconciled, Saved, Sealed, connection::Answer};
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

pub const OFFLINE_CLIENT: ClientId = 0;

const JOINING_OWNER: ClientId = ClientId::MAX;

pub const EARLY_OPEN: std::time::Duration = std::time::Duration::from_millis(250);

fn offline_session(head: Option<CommitId>) -> SessionState {
    SessionState {
        generation: 0,
        owner: Some(OFFLINE_CLIENT),
        participants: vec![OFFLINE_CLIENT],
        clean_at: head,
    }
}

fn joining_session(head: Option<CommitId>) -> SessionState {
    SessionState {
        generation: 0,
        owner: Some(JOINING_OWNER),
        participants: vec![JOINING_OWNER, OFFLINE_CLIENT],
        clean_at: head,
    }
}

type Joined = Result<(ClientId, SessionState, Option<CommitId>, Option<CommitId>), ClientError>;

type Rejoined = Result<(ClientId, SessionState, Option<CommitId>), ClientError>;

type Pending<T> = std::pin::Pin<Box<dyn Future<Output = T>>>;

type Waiting = tokio::sync::oneshot::Sender<Outcome>;

pub(crate) fn answer_in(
    answered: &mut tokio::sync::oneshot::Receiver<Outcome>,
) -> Option<Result<Outcome, OpError>> {
    match answered.try_recv() {
        Ok(outcome) => Some(Ok(outcome)),
        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => Some(Err(OpError::Canceled)),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => None,
    }
}

fn unreachable(error: &ClientError) -> bool {
    error.is_unreachable()
}

const PUBLISHING: u8 = 0;
const RELEASED: u8 = 1;
const SETTLED: u8 = 2;

#[derive(Default)]
struct Publishing {
    state: std::sync::atomic::AtomicU8,
    kept: Mutex<Option<CommitId>>,
}

impl Publishing {
    fn keep(&self, commit: CommitId) {
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner) = Some(commit);
    }

    fn kept(&self) -> Option<CommitId> {
        *self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn release(&self) -> bool {
        self.moved(RELEASED)
    }

    fn settle(&self) -> bool {
        self.moved(SETTLED)
    }

    fn moved(&self, to: u8) -> bool {
        use std::sync::atomic::Ordering::SeqCst;
        self.state
            .compare_exchange(PUBLISHING, to, SeqCst, SeqCst)
            .is_ok()
    }
}

struct Unanswered {
    replica: ReplicaId,
    module: usize,
    generation: u64,
    epoch: u64,
    reply: Reply<Result<PublishOutcome, String>>,
    publishing: Arc<Publishing>,
}

pub enum Answered {
    Read(Reply<Result<Vec<u8>, String>>, Result<Vec<u8>, String>),
    Stash(Reply<Result<CommitId, String>>, Result<CommitId, String>),
    Resume(Reply<Result<Resume, String>>, Result<Resume, String>),
}

impl Answered {
    fn heads(&self) -> bool {
        matches!(self, Answered::Stash(..))
    }

    fn send(self, connection: &mut Connection) {
        match self {
            Answered::Read(reply, result) => reply.send(connection, &result),
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
    Published {
        publish: u64,
        result: Result<PublishOutcome, String>,
        published: Option<CommitId>,
        kept: bool,
    },
    Relayed,
    Left,
    Sealed,
    Keyed {
        block: Uuid,
        error: Option<ClientError>,
    },
    Joined {
        replica: ReplicaId,
        block: Uuid,
        joined: Box<Joined>,
    },
    Rejoined {
        replica: ReplicaId,
        block: Uuid,
        rejoin: u64,
        joined: Box<Rejoined>,
    },
    Claimed {
        replica: ReplicaId,
        claimed: Box<Result<SessionState, ClientError>>,
    },
    Resynchronized {
        replica: ReplicaId,
        joined: Box<Result<(ClientId, SessionState), ClientError>>,
    },
}

#[derive(Clone, Copy)]
enum Asked {
    Content,
    Seal(bool),
}

impl Asked {
    fn send(self, replica: Replica, connection: &mut Connection) -> OpId {
        match self {
            Asked::Content => replica.content(connection).op(),
            Asked::Seal(force) => replica.seal(connection, force).op(),
        }
    }
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
    Change {
        change: Change,
        op: Option<OpId>,
    },
}

impl Replayed {
    fn local_edit(&self) -> bool {
        matches!(
            self,
            Replayed::Message {
                from: Endpoint::Editor(_),
                ..
            } | Replayed::Change { .. }
        )
    }

    fn send(&self, replica: Replica, connection: &mut Connection) -> Option<OpId> {
        match self.clone() {
            Replayed::Change { change, .. } => {
                return Some(replica.change(connection, change).op());
            }
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
        None
    }
}

fn resend(
    handle: Replica,
    connection: &mut Connection,
    module: usize,
    replica: ReplicaId,
    replay: &mut [Replayed],
    asked: Vec<(Asked, Option<Waiting>)>,
    waiting: &mut HashMap<OpId, Waiting>,
    replies: &mut HashMap<(usize, OpId), Waiting>,
    pending: &mut HashMap<(usize, OpId), (ReplicaId, Asked)>,
) {
    for replayed in replay.iter_mut() {
        let sent = replayed.send(handle, connection);
        if let (Replayed::Change { op, .. }, Some(sent)) = (replayed, sent) {
            if let Some(answer) = op.and_then(|op| waiting.remove(&op)) {
                replies.insert((module, sent), answer);
            }
            *op = Some(sent);
        }
    }
    for (request, answer) in asked {
        let sent = request.send(handle, connection);
        pending.insert((module, sent), (replica, request));
        if let Some(answer) = answer {
            replies.insert((module, sent), answer);
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
    joining: bool,
    rejoining: Option<u64>,
    made_up: bool,
    early: Vec<Replayed>,
    unreported: bool,
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
    replies: HashMap<(usize, OpId), Waiting>,
    asked: HashMap<(usize, OpId), (ReplicaId, Asked)>,
    pending: Vec<Io>,
    relaying: usize,
    left: usize,
    unsealing: HashMap<Uuid, VecDeque<ServerMessage>>,
    leaving: Vec<Uuid>,
    rejoins: u64,
    publishes: HashMap<u64, Unanswered>,
    published: u64,
    keeping: bool,
    kept: bool,
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
            asked: HashMap::new(),
            pending: Vec::new(),
            relaying: 0,
            left: 0,
            unsealing: HashMap::new(),
            leaving: Vec::new(),
            rejoins: 0,
            publishes: HashMap::new(),
            published: 0,
            keeping: false,
            kept: false,
            changed: false,
            handled: 0,
            host: std::marker::PhantomData,
        }
    }

    pub fn relaying(&self) -> bool {
        self.relaying > 0
    }

    pub fn leaving(&self) -> bool {
        self.left > 0
    }

    fn leave(&mut self, peer: &Peer<H::Store>, block: Uuid) {
        let left = peer.leave_session(block);
        let unwatched = peer.unwatch(block);
        self.left += 1;
        let io = self.io.clone();
        H::spawn(async move {
            if let Ok(left) = left {
                let _ = left.await;
            }
            if let Ok(unwatched) = unwatched {
                let _ = unwatched.await;
            }
            let _ = io.send(Io::Left);
            H::wake();
        });
    }

    fn relay(&mut self, shared: &Arc<Mutex<H>>, sent: Result<Answer, ClientError>) {
        let answer = match sent {
            Ok(answer) => answer,
            Err(error) if unreachable(&error) => return,
            Err(error) => return H::record(shared, error),
        };
        self.relaying += 1;
        let shared = Arc::clone(shared);
        let io = self.io.clone();
        H::spawn(async move {
            if let Err(error) = answer.await
                && !unreachable(&error)
            {
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
                self.answered(ctx, io);
            }
            for block in std::mem::take(&mut self.leaving) {
                self.leave(ctx.0, block);
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
        self.wait(ctx, ticket, answered).await
    }

    async fn wait<T: DeserializeOwned>(
        &mut self,
        ctx: Ctx<'_, H>,
        ticket: Ticket<T>,
        mut answered: tokio::sync::oneshot::Receiver<Outcome>,
    ) -> Result<T, OpError> {
        loop {
            self.drain(ctx).await;
            if let Some(outcome) = answer_in(&mut answered) {
                return outcome.and_then(|outcome| ticket.decode(outcome));
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
        let known = peer
            .known_summary(block)
            .is_some_and(|summary| summary.content_type == content_type);
        let joined = if known
            && peer.is_attached()
            && !(owned && H::reshaped(shared, block, &kind.schema))
        {
            let joining: Pending<Joined> = Box::pin({
                let peer = Arc::clone(peer);
                async move {
                    peer.ensure_type(block, content_type, BlockParent::Detached)
                        .await?;
                    let (client, session) = peer.join_session(block)?.await?;
                    peer.watch(block).await?;
                    let head = peer.remote_head(block).await?;
                    Ok((client, session, head, ahead(&peer, block)))
                }
            });
            let waited = self.modules.clock().sleep(EARLY_OPEN);
            match futures_util::future::select(joining, waited).await {
                futures_util::future::Either::Left((joined, _)) => {
                    joined.map(|(client, session, head, _)| (client, session, head, false))
                }
                futures_util::future::Either::Right((_, joining)) => {
                    let head = known_head_of(peer, block);
                    if let Some(replica) = self.start(
                        peer,
                        block,
                        content_type,
                        reader,
                        OFFLINE_CLIENT,
                        joining_session(head),
                        head,
                    ) {
                        self.join_later(replica, block, joining);
                    }
                    return;
                }
            }
        } else {
            async {
                if peer.known_summary(block).is_none() || peer.is_attached() {
                    peer.ensure_type(block, content_type, BlockParent::Detached)
                        .await?;
                }
                if !peer.is_attached() {
                    return Ok((OFFLINE_CLIENT, offline_session(None), None, true));
                }
                if owned && H::reshaped(shared, block, &kind.schema) {
                    reset::<H>(peer, block, &kind).await?;
                }
                let (client, session) = peer.join_session(block)?.await?;
                peer.watch(block).await?;
                let head = peer.remote_head(block).await?;
                Ok::<_, ClientError>((client, session, head, false))
            }
            .await
        };
        let joined = match joined {
            Err(error) if unreachable(&error) && peer.known_summary(block).is_some() => {
                Ok((OFFLINE_CLIENT, offline_session(None), None, true))
            }
            joined => joined,
        };
        let (client, session, head, offline) = match joined {
            Ok((client, _, _, true)) => {
                let head = known_head_of(peer, block);
                (client, offline_session(head), head, true)
            }
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
        let remote = head;
        let head = match offline {
            true => head,
            false => ahead(peer, block).or(head),
        };
        let Some(replica) = self.start(peer, block, content_type, reader, client, session, head)
        else {
            return;
        };
        if let Some(open) = self.open.get_mut(&replica) {
            open.made_up = offline;
        }
        if offline {
            return;
        }
        let mut heads = vec![remote, H::known_head(shared, block)];
        heads.dedup();
        for moved in heads.into_iter().flatten() {
            if Some(moved) != head {
                self.moved(replica, moved, false);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start(
        &mut self,
        peer: &Arc<Peer<H::Store>>,
        block: Uuid,
        content_type: Uuid,
        reader: bool,
        client: ClientId,
        session: SessionState,
        head: Option<CommitId>,
    ) -> Option<ReplicaId> {
        if self.blocks.contains_key(&block) {
            return None;
        }
        let module = self.modules.module_of(content_type)?;
        self.next += 1;
        let replica = self.next;
        let organization = peer.organization();
        let joining = session.owner == Some(JOINING_OWNER);
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
        let handle = self
            .modules
            .call(module, |connection| Replica::new(connection, opening))?;
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
                joining,
                rejoining: None,
                made_up: joining,
                early: Vec::new(),
                unreported: false,
            },
        );
        if reader {
            self.attach_now(replica, HOST_EDITOR, true);
        }
        Some(replica)
    }

    fn join_later(&mut self, replica: ReplicaId, block: Uuid, joining: Pending<Joined>) {
        let io = self.io.clone();
        H::spawn(async move {
            let joined = joining.await;
            let _ = io.send(Io::Joined {
                replica,
                block,
                joined: Box::new(joined),
            });
            H::wake();
        });
    }

    pub fn joining(&self) -> bool {
        self.open.values().any(|open| open.joining)
    }

    fn joined(&mut self, shared: &Arc<Mutex<H>>, replica: ReplicaId, block: Uuid, joined: Joined) {
        let Some(open) = self.open.get_mut(&replica) else {
            if joined.is_ok() && !self.blocks.contains_key(&block) {
                self.leaving.push(block);
            }
            return;
        };
        if !open.joining || open.rejoining.is_some() {
            return;
        }
        let block = open.block;
        let (client, session, head, ahead) = match joined {
            Ok(joined) => joined,
            Err(error) if unreachable(&error) => return,
            Err(error) => {
                if let ClientError::Locked(_) = &error {
                    H::set_locked(shared, block);
                } else {
                    H::record(shared, error);
                }
                open.joining = false;
                self.make_up_session(replica);
                self.close_if_asked(replica);
                return;
            }
        };
        open.joining = false;
        open.made_up = false;
        let edited = open.replay.iter().any(Replayed::local_edit);
        let generation = session.generation;
        let early: Vec<Replayed> = std::mem::take(&mut open.early)
            .into_iter()
            .filter(|replayed| match replayed {
                Replayed::Lease { session, .. } => session.generation >= generation,
                _ => true,
            })
            .collect();
        let from = match edited || ahead.is_some() {
            true => open.status.head,
            false => head,
        };
        self.reopen_joined(replica, client, session, from);
        for replayed in early {
            self.replay(replica, replayed);
        }
        let mut heads = vec![head, H::known_head(shared, block)];
        heads.dedup();
        for moved in heads.into_iter().flatten() {
            if Some(moved) != from {
                self.moved(replica, moved, false);
            }
        }
        self.close_if_asked(replica);
    }

    fn reopen_joined(
        &mut self,
        replica: ReplicaId,
        client: ClientId,
        session: SessionState,
        head: Option<CommitId>,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.client = client;
        open.session = session.clone();
        open.epoch += 1;
        open.publishing = 0;
        open.moved.clear();
        open.published.clear();
        open.status.head = head;
        open.replay.retain(Replayed::local_edit);
        let (module, handle) = (open.module, open.handle);
        let opening = Opening {
            block: open.block,
            content_type: open.content_type,
            organization: open.organization,
            client,
            tag: self.modules.tag(module),
            head,
            session,
        };
        let editors: Vec<(EditorId, bool)> = open
            .editors
            .iter()
            .map(|(editor, editable)| (*editor, *editable))
            .collect();
        let mut asked = Vec::new();
        let mut waiting = HashMap::new();
        for key in self
            .asked
            .iter()
            .filter(|(_, (to, _))| *to == replica)
            .map(|(key, _)| *key)
            .collect::<Vec<_>>()
        {
            if let Some((_, request)) = self.asked.remove(&key) {
                asked.push((request, self.replies.remove(&key)));
            }
        }
        for replayed in &open.replay {
            if let Replayed::Change { op: Some(op), .. } = replayed
                && let Some(answer) = self.replies.remove(&(module, *op))
            {
                waiting.insert(*op, answer);
            }
        }
        let (replies, pending, replay) = (&mut self.replies, &mut self.asked, &mut open.replay);
        self.modules.call(module, |connection| {
            handle.reopen(connection, opening);
            for (editor, editable) in editors {
                handle.attach(connection, editor, editable);
            }
            resend(
                handle,
                connection,
                module,
                replica,
                replay,
                asked,
                &mut waiting,
                replies,
                pending,
            );
        });
    }

    fn make_up_session(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        open.made_up = true;
        open.session = SessionState {
            generation: open.session.generation + 1,
            owner: Some(open.client),
            participants: vec![open.client],
            clean_at: None,
        };
        let session = open.session.clone();
        self.with_replica(replica, |handle, connection| {
            handle.lease(connection, session, false);
        });
    }

    fn close_if_asked(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        if open.closing && !open.joining {
            self.with_replica(replica, |handle, connection| {
                handle.resource_drop(connection)
            });
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
        if let Some(open) = self.open.get_mut(&replica) {
            open.unreported = true;
        }
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
        if std::mem::replace(&mut open.closing, true) || open.joining {
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
        if std::mem::replace(&mut open.closing, true) || open.joining {
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
        asked: Asked,
    ) -> Result<T, OpError> {
        let Some((replica, open)) = self.replica_of(block) else {
            return Err(OpError::Canceled);
        };
        let (module, handle) = (open.module, open.handle);
        let Some((ticket, answered)) = self.ask_later(module, |connection| {
            Ticket::<T>::new(asked.send(handle, connection))
        }) else {
            return Err(OpError::Canceled);
        };
        self.asked.insert((module, ticket.op()), (replica, asked));
        self.wait(ctx, ticket, answered).await
    }

    fn change_now(&mut self, block: Uuid, change: Change) -> Option<(usize, OpId)> {
        let (replica, open) = self.replica_of(block)?;
        let (module, handle) = (open.module, open.handle);
        let replayed = Replayed::Change { change, op: None };
        let op = self
            .modules
            .call(module, |connection| replayed.send(handle, connection))
            .flatten()?;
        let open = self.open.get_mut(&replica)?;
        open.replay.push(match replayed {
            Replayed::Change { change, .. } => Replayed::Change {
                change,
                op: Some(op),
            },
            replayed => replayed,
        });
        Some((module, op))
    }

    pub fn change_later(&mut self, block: Uuid, change: Change) {
        self.change_now(block, change);
    }

    pub async fn content(&mut self, ctx: Ctx<'_, H>, block: Uuid) -> Option<Vec<u8>> {
        self.on_replica::<Option<Vec<u8>>>(ctx, block, Asked::Content)
            .await
            .ok()
            .flatten()
    }

    pub async fn change(&mut self, ctx: Ctx<'_, H>, block: Uuid, change: Change) -> bool {
        let Some((module, op)) = self.change_now(block, change) else {
            return false;
        };
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert((module, op), answer);
        self.wait(ctx, Ticket::<bool>::new(op), answered)
            .await
            .unwrap_or(false)
    }

    pub async fn seal(&mut self, ctx: Ctx<'_, H>, block: Uuid, force: bool) {
        if self.ready_to_seal(block) {
            let _ = self.on_replica::<()>(ctx, block, Asked::Seal(force)).await;
        }
    }

    pub fn seal_later(&mut self, block: Uuid, force: bool) {
        if !self.ready_to_seal(block) {
            return;
        }
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        let (module, handle) = (open.module, open.handle);
        let asked = Asked::Seal(force);
        if let Some(op) = self
            .modules
            .call(module, |connection| asked.send(handle, connection))
        {
            self.asked.insert((module, op), (replica, asked));
        }
    }

    fn ready_to_seal(&mut self, block: Uuid) -> bool {
        if let Some((replica, open)) = self.replica_of(block)
            && open.joining
            && open.session.owner == Some(JOINING_OWNER)
        {
            if open.status.clean {
                return false;
            }
            self.make_up_session(replica);
        }
        true
    }

    pub async fn seal_all(&mut self, ctx: Ctx<'_, H>) {
        for block in self.open_blocks() {
            self.seal(ctx, block, false).await;
        }
    }

    pub async fn keep_all(&mut self, ctx: Ctx<'_, H>) {
        self.keeping = true;
        self.release();
        self.seal_all(ctx).await;
        self.keeping = false;
    }

    pub fn take_kept(&mut self) -> bool {
        std::mem::take(&mut self.kept)
    }

    fn release(&mut self) {
        if !self.keeping {
            return;
        }
        let ready: Vec<(u64, CommitId)> = self
            .publishes
            .iter()
            .filter_map(|(publish, unanswered)| Some((*publish, unanswered.publishing.kept()?)))
            .collect();
        for (publish, commit) in ready {
            if !self.publishes[&publish].publishing.release() {
                continue;
            }
            if let Some(unanswered) = self.publishes.remove(&publish) {
                self.kept = true;
                self.answer_publish(unanswered, Ok(PublishOutcome::Published(commit)), None);
            }
        }
    }

    fn answer_publish(
        &mut self,
        unanswered: Unanswered,
        result: Result<PublishOutcome, String>,
        published: Option<CommitId>,
    ) {
        let Unanswered {
            replica,
            module,
            generation,
            epoch,
            reply,
            ..
        } = unanswered;
        if self.modules.generation(module) == generation {
            self.modules
                .call(module, |connection| reply.send(connection, &result));
        }
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if open.epoch != epoch {
            return;
        }
        open.publishing = open.publishing.saturating_sub(1);
        if let Some(head) = published {
            open.published.push(head);
        }
        if open.publishing == 0 {
            for head in std::mem::take(&mut open.moved) {
                self.moved(replica, head, false);
            }
        }
    }

    pub async fn merge(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        base: Option<&[u8]>,
        ours: Option<&[u8]>,
        theirs: Option<&[u8]>,
    ) -> Option<(Vec<u8>, usize)> {
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
            Ok(Ok(merged)) => Some((merged.bytes, merged.conflicts as usize)),
            _ => None,
        }
    }

    pub async fn merge_diverged(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        remote: Option<CommitId>,
    ) -> Result<Option<CommitId>, ClientError> {
        let Some(content_type) = peer
            .known_summary(block)
            .map(|summary| summary.content_type)
        else {
            return Ok(None);
        };
        let mut remote = remote;
        for _ in 0..4 {
            let (base, ours, theirs) = match peer.reconcile(block, remote).await? {
                Reconciled::Done => return Ok(peer.known_remote_head(block)),
                Reconciled::Merge { base, ours, theirs } => (base, ours, theirs),
            };
            let mut sides = Vec::new();
            for commit in [base, Some(ours), Some(theirs)] {
                sides.push(match commit {
                    Some(commit) => {
                        let commit = peer.load_commit(commit).await?;
                        Some(peer.read_manifest(&commit.manifest).await?)
                    }
                    None => None,
                });
            }
            let Some((bytes, conflicts)) = self
                .merge(
                    (peer, shared),
                    content_type,
                    sides[0].as_deref(),
                    sides[1].as_deref(),
                    sides[2].as_deref(),
                )
                .await
            else {
                return Ok(None);
            };
            let description = self
                .describe((peer, shared), content_type, &bytes)
                .await
                .unwrap_or_default();
            let saved = peer
                .publish_encoded(
                    block,
                    content_type,
                    &bytes,
                    description.references,
                    description.derived,
                    Some(theirs),
                    Some((base, ours)),
                )
                .await?;
            match saved {
                Saved::Published(head) | Saved::Unchanged(head) => {
                    if conflicts > 0 {
                        H::conflict(
                            shared,
                            block,
                            crate::Conflict {
                                ours,
                                theirs,
                                count: conflicts,
                            },
                        );
                    }
                    remote = Some(head);
                }
                Saved::Rejected { head } => remote = head,
            }
        }
        Err(ClientError::Contended)
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

    pub async fn from_file(
        &mut self,
        ctx: Ctx<'_, H>,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<(Uuid, Vec<u8>), String> {
        let mut claimed = None;
        for (module, claim) in self.modules.claim_requests(name, &bytes, false) {
            if let Ok(Some(content_type)) = self
                .ask(ctx, module, |connection| claim.call(connection))
                .await
            {
                claimed = Some(content_type);
                break;
            }
        }
        let content_type = claimed.ok_or_else(|| format!("No block type takes {name}."))?;
        let change = Change::FromFile(block_type_api::FileChange {
            name: name.to_owned(),
            bytes,
        });
        let content = self
            .apply(ctx, content_type, None, 0, change)
            .await?
            .ok_or_else(|| format!("{name} has no block form."))?;
        Ok((content_type, content))
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

    pub fn heard(&mut self, (peer, shared): Ctx<'_, H>, event: &ServerMessage) {
        let (ServerMessage::SessionChanged { block, .. }
        | ServerMessage::Relayed { block, .. }
        | ServerMessage::HeadChanged { block, .. }) = event
        else {
            return;
        };
        if let Some(waiting) = self.unsealing.get_mut(block) {
            waiting.push_back(event.clone());
            return;
        }
        match event {
            ServerMessage::SessionChanged { block, state } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                let replayed = Replayed::Lease {
                    session: state.clone(),
                    resynchronize: false,
                };
                if open.joining {
                    return open.early.push(replayed);
                }
                open.session = state.clone();
                self.replay(replica, replayed);
            }
            ServerMessage::Relayed {
                block,
                from,
                payload,
            } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let bytes = match peer.unseal(payload) {
                    Ok(bytes) => bytes,
                    Err(ClientError::Locked(key)) if peer.first_search_for(key) => {
                        self.unsealing
                            .insert(*block, VecDeque::from([event.clone()]));
                        let (peer, io, block) = (Arc::clone(peer), self.io.clone(), *block);
                        H::spawn(async move {
                            let error = peer.refresh_keys(None).await.err();
                            let _ = io.send(Io::Keyed { block, error });
                            H::wake();
                        });
                        return;
                    }
                    Err(error) => return H::record(shared, error),
                };
                let replayed = Replayed::Message {
                    from: Endpoint::Peer(*from),
                    bytes,
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(replayed);
                }
                self.replay(replica, replayed);
            }
            ServerMessage::HeadChanged { block, head, .. } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(Replayed::Moved {
                        head: *head,
                        from_this_device: false,
                    });
                }
                self.moved(replica, *head, false);
            }
            _ => {}
        }
    }

    pub fn resynchronize(&mut self, (peer, shared): Ctx<'_, H>) {
        for (replica, block) in self
            .open
            .iter()
            .filter(|(_, open)| !open.joining)
            .map(|(replica, open)| (*replica, open.block))
            .collect::<Vec<_>>()
        {
            let joining = match peer.join_session(block) {
                Ok(joining) => joining,
                Err(error) => {
                    H::record(shared, error);
                    continue;
                }
            };
            let io = self.io.clone();
            H::spawn(async move {
                let joined = joining.await;
                let _ = io.send(Io::Resynchronized {
                    replica,
                    joined: Box::new(joined),
                });
                H::wake();
            });
        }
    }

    fn resynchronized(
        &mut self,
        shared: &Arc<Mutex<H>>,
        replica: ReplicaId,
        joined: Result<(ClientId, SessionState), ClientError>,
    ) {
        let session = match joined {
            Ok((_, session)) => session,
            Err(error) if unreachable(&error) => return,
            Err(error) => return H::record(shared, error),
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if open.joining || session.generation < open.session.generation {
            return;
        }
        open.session = session.clone();
        self.with_replica(replica, |handle, connection| {
            handle.lease(connection, session, true);
        });
    }

    fn claimed(
        &mut self,
        shared: &Arc<Mutex<H>>,
        replica: ReplicaId,
        claimed: Result<SessionState, ClientError>,
    ) {
        let state = match claimed {
            Ok(state) => state,
            Err(error) if unreachable(&error) => return,
            Err(error) => return H::record(shared, error),
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let replayed = Replayed::Lease {
            session: state.clone(),
            resynchronize: false,
        };
        if open.joining {
            return open.early.push(replayed);
        }
        if state.generation < open.session.generation {
            return;
        }
        open.session = state;
        self.replay(replica, replayed);
    }

    fn answered(&mut self, (peer, shared): Ctx<'_, H>, io: Io) {
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
            Io::Published {
                publish,
                result,
                published,
                kept,
            } => {
                self.kept |= kept;
                if let Some(unanswered) = self.publishes.remove(&publish) {
                    self.answer_publish(unanswered, result, published);
                }
            }
            Io::Relayed => self.relaying -= 1,
            Io::Left => self.left -= 1,
            Io::Sealed => self.release(),
            Io::Keyed { block, error } => {
                if let Some(error) = error
                    && !unreachable(&error)
                {
                    H::record(shared, error);
                }
                for event in self.unsealing.remove(&block).unwrap_or_default() {
                    self.heard((peer, shared), &event);
                }
            }
            Io::Joined {
                replica,
                block,
                joined,
            } => self.joined(shared, replica, block, *joined),
            Io::Rejoined {
                replica,
                block,
                rejoin,
                joined,
            } => self.rejoined(shared, replica, block, rejoin, *joined),
            Io::Claimed { replica, claimed } => self.claimed(shared, replica, *claimed),
            Io::Resynchronized { replica, joined } => self.resynchronized(shared, replica, *joined),
        }
    }

    pub fn disconnected(&mut self) {
        let replicas: Vec<ReplicaId> = self.open.keys().copied().collect();
        for replica in replicas {
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            let joining = std::mem::replace(&mut open.joining, false);
            open.rejoining = None;
            open.early.clear();
            self.make_up_session(replica);
            if joining {
                self.close_if_asked(replica);
            }
        }
    }

    pub fn reconnect(&mut self, (peer, _): Ctx<'_, H>) {
        self.rejoins += 1;
        let rejoin = self.rejoins;
        for (replica, open) in &mut self.open {
            open.joining = true;
            open.rejoining = Some(rejoin);
            open.early.clear();
            let (replica, block) = (*replica, open.block);
            let peer = Arc::clone(peer);
            let io = self.io.clone();
            H::spawn(async move {
                let joined = async {
                    let (client, session) = peer.join_session(block)?.await?;
                    peer.watch(block).await?;
                    let head = peer.remote_head(block).await?;
                    Ok::<_, ClientError>((client, session, head))
                }
                .await;
                let _ = io.send(Io::Rejoined {
                    replica,
                    block,
                    rejoin,
                    joined: Box::new(joined),
                });
                H::wake();
            });
        }
    }

    fn rejoined(
        &mut self,
        shared: &Arc<Mutex<H>>,
        replica: ReplicaId,
        block: Uuid,
        rejoin: u64,
        joined: Rejoined,
    ) {
        let Some(open) = self.open.get_mut(&replica) else {
            if joined.is_ok() && rejoin == self.rejoins && !self.blocks.contains_key(&block) {
                self.leaving.push(block);
            }
            return;
        };
        if open.rejoining != Some(rejoin) {
            return;
        }
        open.joining = false;
        open.rejoining = None;
        let early = std::mem::take(&mut open.early);
        let (client, session, head) = match joined {
            Ok(joined) => joined,
            Err(error) => {
                H::record(shared, error);
                for replayed in early {
                    self.replay(replica, replayed);
                }
                self.close_if_asked(replica);
                return;
            }
        };
        open.made_up = false;
        let generation = session.generation;
        let early: Vec<Replayed> = early
            .into_iter()
            .filter(|replayed| match replayed {
                Replayed::Lease { session, .. } => session.generation >= generation,
                _ => true,
            })
            .collect();
        let known = open.status.head;
        self.reopen_joined(replica, client, session, known);
        for replayed in early {
            self.replay(replica, replayed);
        }
        if let Some(head) = head
            && Some(head) != known
        {
            self.moved(replica, head, false);
        }
        self.close_if_asked(replica);
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
        let mut waiting: HashMap<OpId, Waiting> = HashMap::new();
        for key in self
            .replies
            .keys()
            .filter(|(from, _)| *from == module)
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(answer) = self.replies.remove(&key) {
                waiting.insert(key.1, answer);
            }
        }
        let mut asked: HashMap<ReplicaId, Vec<(Asked, Option<Waiting>)>> = HashMap::new();
        for key in self
            .asked
            .keys()
            .filter(|(from, _)| *from == module)
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some((replica, request)) = self.asked.remove(&key) {
                asked
                    .entry(replica)
                    .or_default()
                    .push((request, waiting.remove(&key.1)));
            }
        }
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
            let closing = open.closing && !open.joining;
            let asked = asked.remove(&replica).unwrap_or_default();
            let (replies, pending, replay) = (&mut self.replies, &mut self.asked, &mut open.replay);
            let Some(handle) = self.modules.call(module, |connection| {
                let handle = Replica::new(connection, opening);
                for (editor, editable) in editors {
                    handle.attach(connection, editor, editable);
                }
                resend(
                    handle,
                    connection,
                    module,
                    replica,
                    replay,
                    asked,
                    &mut waiting,
                    replies,
                    pending,
                );
                if closing {
                    handle.resource_drop(connection);
                }
                handle
            }) else {
                continue;
            };
            open.handle = handle;
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
                            H::report(ctx.1, None, format!("a type module: {message}"));
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
        self.asked.remove(&(module, op));
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
        self.asked.retain(|_, (to, _)| *to != replica);
        self.blocks.remove(&open.block);
        if !open.joining {
            self.leave(peer, open.block);
        }
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
        let Some((block, content_type, made_up, joining)) = self
            .open
            .get(&replica)
            .map(|open| (open.block, open.content_type, open.made_up, open.joining))
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
                _ if made_up => false,
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
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                open.publishing += 1;
                let epoch = open.epoch;
                let local = made_up || self.keeping || !peer.pending_commits(block).is_empty();
                let publishing = Arc::new(Publishing::default());
                self.published += 1;
                let publish = self.published;
                self.publishes.insert(
                    publish,
                    Unanswered {
                        replica,
                        module,
                        generation: self.modules.generation(module),
                        epoch,
                        reply,
                        publishing: Arc::clone(&publishing),
                    },
                );
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                H::spawn(async move {
                    let sealed = io.clone();
                    let saved = commit::<H>(
                        &peer,
                        block,
                        content_type,
                        content,
                        expected,
                        merging.map(|merging| (merging.base, merging.ours)),
                        local,
                        &publishing,
                        move || {
                            let _ = sealed.send(Io::Sealed);
                        },
                    )
                    .await;
                    let kept = publishing
                        .kept()
                        .is_some_and(|commit| peer.pending_commits(block).contains(&commit));
                    let (result, published) = match saved {
                        Ok(Saved::Published(head)) => {
                            (Ok(PublishOutcome::Published(head)), Some(head))
                        }
                        Ok(Saved::Unchanged(head)) => (Ok(PublishOutcome::Published(head)), None),
                        Ok(Saved::Rejected { head }) => (Ok(PublishOutcome::Rejected(head)), None),
                        Err(error) => (Err(error.to_string()), None),
                    };
                    let _ = io.send(Io::Published {
                        publish,
                        result,
                        published,
                        kept,
                    });
                    H::wake();
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
                        let remote = match peer.is_attached() && !joining {
                            true => peer.remote_head(block).await?,
                            false => match peer.local_head(block) {
                                Some(head) => Some(head),
                                None => peer.remote_head(block).await?,
                            },
                        };
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
                if !peer.is_attached() || made_up {
                    return false;
                }
                let claiming = match peer.claim_ownership(block, generation) {
                    Ok(claiming) => claiming,
                    Err(error) if unreachable(&error) => return false,
                    Err(error) => {
                        H::record(shared, error);
                        return false;
                    }
                };
                let io = self.io.clone();
                H::spawn(async move {
                    let claimed = claiming.await;
                    let _ = io.send(Io::Claimed {
                        replica,
                        claimed: Box::new(claimed),
                    });
                    H::wake();
                });
                false
            }
            Call::ReportClean {
                generation,
                clean_at,
            } => {
                if !peer.is_attached() || made_up {
                    return false;
                }
                let answer = match peer.heartbeat(block, generation, clean_at) {
                    Ok(answer) => answer,
                    Err(error) if unreachable(&error) => return false,
                    Err(error) => {
                        H::record(shared, error);
                        return false;
                    }
                };
                let shared = Arc::clone(shared);
                H::spawn(async move {
                    if let Err(error) = answer.await
                        && !unreachable(&error)
                    {
                        H::record(&shared, error);
                    }
                });
                false
            }
            Call::ReportStatus { status } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                open.unreported = false;
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
                H::report(shared, Some(block), format!("{block}: {message}"));
                false
            }
        }
    }

    pub fn counts(&self) -> (usize, usize) {
        let unsealed = self
            .open
            .values()
            .filter(|open| !open.status.clean || open.unreported)
            .count();
        let owing = self
            .open
            .values()
            .filter(|open| open.status.owes_seal)
            .count();
        (unsealed, owing)
    }

    fn counted(&self, shared: &Arc<Mutex<H>>) {
        let (unsealed, owing) = self.counts();
        H::counted(shared, unsealed, owing);
    }
}

fn known_head_of<S: be_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Option<CommitId> {
    peer.local_head(block)
        .or_else(|| peer.known_remote_head(block))
        .or_else(|| peer.known_summary(block).and_then(|summary| summary.head))
}

#[allow(clippy::too_many_arguments)]
async fn commit<H: Host>(
    peer: &Peer<H::Store>,
    block: Uuid,
    content_type: Uuid,
    content: block_type_api::Written,
    expected: Option<CommitId>,
    merging: Option<(Option<CommitId>, CommitId)>,
    local: bool,
    publishing: &Publishing,
    sealed: impl FnOnce(),
) -> Result<Saved, ClientError> {
    let before = peer.local_head(block);
    let commit = match peer
        .seal_encoded(
            block,
            content_type,
            &content.bytes,
            content.references,
            content.derived,
            expected,
            merging,
        )
        .await?
    {
        Sealed::Unchanged(head) => return Ok(Saved::Unchanged(head)),
        Sealed::Sealed(commit) => commit,
    };
    publishing.keep(commit);
    sealed();
    if local || !peer.is_attached() {
        return Ok(Saved::Unchanged(commit));
    }
    let saved = peer.publish_sealed(block, commit).await;
    if !publishing.settle() {
        return Ok(Saved::Unchanged(commit));
    }
    match saved {
        Ok(Saved::Rejected { head }) => {
            peer.withdraw(block, commit, before)?;
            Ok(Saved::Rejected { head })
        }
        Err(error) if unreachable(&error) => Ok(Saved::Unchanged(commit)),
        saved => saved,
    }
}

fn ahead<S: be_store::ObjectStore>(peer: &Peer<S>, block: Uuid) -> Option<CommitId> {
    match peer.pending_commits(block).is_empty() {
        true => None,
        false => peer.local_head(block),
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
