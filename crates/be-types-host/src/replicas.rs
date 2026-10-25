use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Instant,
};

use wasi_threads::Mutex;

use be_client::{ClientError, Peer, Saved, Sealed, connection::Answer};
use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::{ClientId, ServerMessage, SessionState};
use block_type_api::{
    ByteRange, Change, Command, EditorId, Endpoint, Event, HeadEvent, Recipients, ReplicaId, Reply,
    Request, RequestId, Status,
};
use block_type_guest::editor::FromEditor;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use crate::{
    Host,
    modules::{self as types, Modules},
};

pub const HOST_EDITOR: EditorId = u64::MAX;

pub const OFFLINE_CLIENT: ClientId = 0;

const JOINING_OWNER: ClientId = ClientId::MAX;

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

type Joined = Result<(ClientId, SessionState, Option<CommitId>), ClientError>;

type Pending<T> = std::pin::Pin<Box<dyn Future<Output = T>>>;

pub const EARLY_OPEN: std::time::Duration = std::time::Duration::from_millis(250);

fn asked_of(asked: &HashMap<RequestId, (ReplicaId, Request)>, replica: ReplicaId) -> Vec<Event> {
    let mut asked: Vec<_> = asked
        .iter()
        .filter(|(_, (to, _))| *to == replica)
        .map(|(id, (_, request))| (*id, request.clone()))
        .collect();
    asked.sort_by_key(|(id, _)| *id);
    asked
        .into_iter()
        .map(|(id, request)| Event::Request { id, request })
        .collect()
}

fn local_edit(event: &Event) -> bool {
    matches!(
        event,
        Event::Message {
            from: Endpoint::Editor(_),
            ..
        } | Event::Request {
            request: Request::Change { .. },
            ..
        }
    )
}

fn unreachable(error: &ClientError) -> bool {
    error.is_unreachable()
}

pub enum Io {
    Answer {
        replica: ReplicaId,
        epoch: u64,
        event: Box<Event>,
        published: Option<CommitId>,
    },
    Locked {
        replica: ReplicaId,
    },
    Relayed,
    Sealed,
    Joined {
        replica: ReplicaId,
        block: Uuid,
        joined: Box<Joined>,
    },
}

struct Open {
    block: Uuid,
    module: usize,
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
    replay: Vec<Event>,
    closing: bool,
    reopen: bool,
    epoch: u64,
    joining: bool,
    made_up: bool,
    early: Vec<Event>,
    unreported: bool,
}

pub struct Replicas<H: Host> {
    pub modules: Modules,
    answers: UnboundedReceiver<Io>,
    open: HashMap<ReplicaId, Open>,
    blocks: HashMap<Uuid, ReplicaId>,
    next: ReplicaId,
    io: UnboundedSender<Io>,
    contents: HashMap<RequestId, ReplicaId>,
    replies: HashMap<RequestId, tokio::sync::oneshot::Sender<Reply>>,
    pending: Vec<Io>,
    relaying: usize,
    leaving: Vec<Uuid>,
    asked: HashMap<RequestId, (ReplicaId, Request)>,
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
            contents: HashMap::new(),
            replies: HashMap::new(),
            pending: Vec::new(),
            relaying: 0,
            leaving: Vec::new(),
            asked: HashMap::new(),
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
            let commands = self.modules.take();
            for module in self.modules.take_restarted() {
                self.restart(module).await;
                self.changed = true;
            }
            if answers.is_empty() && commands.is_empty() {
                return;
            }
            self.handled += (answers.len() + commands.len()) as u64;
            for io in answers {
                self.answered(io, ctx.1);
            }
            for block in std::mem::take(&mut self.leaving) {
                let _ = ctx.0.leave_session(block).await;
                let _ = ctx.0.unwatch(block).await;
            }
            for (module, command) in commands {
                self.changed |= self.act(ctx, module, command).await;
            }
        }
    }

    pub async fn ask(&mut self, ctx: Ctx<'_, H>, module: usize, request: Request) -> Reply {
        let Some(answered) = self.ask_later(module, request) else {
            return Reply::Unknown;
        };
        let mut answered = std::pin::pin!(answered);
        loop {
            self.drain(ctx).await;
            if let Some(reply) = futures_util::FutureExt::now_or_never(answered.as_mut()) {
                return reply.unwrap_or(Reply::Unknown);
            }
            self.woken().await;
        }
    }

    fn ask_later(
        &mut self,
        module: usize,
        request: Request,
    ) -> Option<tokio::sync::oneshot::Receiver<Reply>> {
        let id = self.modules.next_request();
        let (answer, answered) = tokio::sync::oneshot::channel();
        self.replies.insert(id, answer);
        match &request {
            Request::Change { replica, .. } => {
                if let Some(open) = self.open.get_mut(replica) {
                    open.replay.push(Event::Request {
                        id,
                        request: request.clone(),
                    });
                }
            }
            Request::Content { replica } | Request::Seal { replica, .. } => {
                self.asked.insert(id, (*replica, request.clone()));
            }
            _ => {}
        }
        self.modules
            .send(module, vec![Event::Request { id, request }]);
        Some(answered)
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
                let module = open.module;
                self.attach_now(module, replica, HOST_EDITOR, true);
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
                    let (client, session) = peer.join_session(block).await?;
                    peer.watch(block).await?;
                    let head = peer.remote_head(block).await?;
                    Ok((client, session, head))
                }
            });
            let waited = self.modules.clock().sleep(EARLY_OPEN);
            match futures_util::future::select(joining, waited).await {
                futures_util::future::Either::Left((joined, _)) => {
                    joined.map(|(client, session, head)| (client, session, head, false))
                }
                futures_util::future::Either::Right((_, joining)) => {
                    let head = peer
                        .local_head(block)
                        .or_else(|| peer.known_remote_head(block))
                        .or_else(|| peer.known_summary(block).and_then(|summary| summary.head));
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
                let (client, session) = peer.join_session(block).await?;
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
                let head = peer
                    .local_head(block)
                    .or_else(|| peer.known_remote_head(block))
                    .or_else(|| peer.known_summary(block).and_then(|summary| summary.head));
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
        let Some(replica) = self.start(peer, block, content_type, reader, client, session, head)
        else {
            return;
        };
        if let Some(open) = self.open.get_mut(&replica) {
            open.made_up = offline;
        }
        let current = H::known_head(shared, block);
        if let Some(current) = current
            && Some(current) != head
            && !offline
        {
            self.moved(replica, current, false);
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
        self.blocks.insert(block, replica);
        self.open.insert(
            replica,
            Open {
                block,
                module,
                content_type,
                client,
                organization,
                editors: BTreeMap::new(),
                status: Status {
                    head,
                    ..Status::default()
                },
                session: session.clone(),
                reader,
                publishing: 0,
                moved: Vec::new(),
                published: Vec::new(),
                replay: Vec::new(),
                closing: false,
                reopen: false,
                epoch: 0,
                joining,
                made_up: joining,
                early: Vec::new(),
                unreported: false,
            },
        );
        let tag = self.modules.tag(module);
        let mut events = vec![Event::Open {
            replica,
            block,
            content_type,
            organization,
            client,
            tag,
            head,
            session,
        }];
        if reader {
            events.push(Event::Attach {
                replica,
                editor: HOST_EDITOR,
                editable: true,
            });
            if let Some(open) = self.open.get_mut(&replica) {
                open.editors.insert(HOST_EDITOR, true);
            }
        }
        self.modules.send(module, events);
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
        if !open.joining {
            return;
        }
        let block = open.block;
        let (client, session, head) = match joined {
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
        let edited = open.replay.iter().any(local_edit);
        let generation = session.generation;
        let early: Vec<Event> = std::mem::take(&mut open.early)
            .into_iter()
            .filter(|event| match event {
                Event::Lease { session, .. } => session.generation >= generation,
                _ => true,
            })
            .collect();
        let from = match edited {
            true => open.status.head,
            false => head,
        };
        self.reopen_joined(replica, client, session, from);
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let module = open.module;
        open.replay.extend(early.iter().cloned());
        self.modules.send(module, early);
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
        open.replay.retain(local_edit);
        let module = open.module;
        let mut events = vec![Event::Open {
            replica,
            block: open.block,
            content_type: open.content_type,
            organization: open.organization,
            client,
            tag: self.modules.tag(module),
            head,
            session,
        }];
        for (editor, editable) in &open.editors {
            events.push(Event::Attach {
                replica,
                editor: *editor,
                editable: *editable,
            });
        }
        events.extend(open.replay.iter().cloned());
        events.extend(asked_of(&self.asked, replica));
        self.modules.send(module, events);
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
        self.modules.send(
            open.module,
            vec![Event::Lease {
                replica,
                session: open.session.clone(),
                resynchronize: false,
            }],
        );
    }

    fn close_if_asked(&mut self, replica: ReplicaId) {
        let Some(open) = self.open.get(&replica) else {
            return;
        };
        if open.closing && !open.joining {
            let module = open.module;
            self.modules.send(module, vec![Event::Close { replica }]);
        }
    }

    fn attach_now(&mut self, module: usize, replica: ReplicaId, editor: EditorId, editable: bool) {
        if let Some(open) = self.open.get_mut(&replica) {
            open.editors.insert(editor, editable);
        }
        self.modules.send(
            module,
            vec![Event::Attach {
                replica,
                editor,
                editable,
            }],
        );
    }

    pub fn attach(&mut self, block: Uuid, editor: EditorId, editable: bool) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        if open.editors.get(&editor) == Some(&editable) {
            return;
        }
        let module = open.module;
        self.attach_now(module, replica, editor, editable);
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
        let module = open.module;
        let idle = open.editors.is_empty();
        self.modules
            .send(module, vec![Event::Detach { replica, editor }]);
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
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        let module = open.module;
        let event = Event::Message {
            replica,
            from: Endpoint::Editor(editor),
            bytes,
        };
        open.replay.push(event.clone());
        open.unreported = true;
        self.modules.tick_at(module, at);
        self.modules.send(module, vec![event]);
    }

    pub fn send_as_host(&mut self, block: Uuid, message: &FromEditor, at: Instant) {
        if let Some((replica, open)) = self.replica_of(block)
            && !open.editors.contains_key(&HOST_EDITOR)
        {
            let module = open.module;
            self.attach_now(module, replica, HOST_EDITOR, true);
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
        let module = open.module;
        self.modules.send(module, vec![Event::Close { replica }]);
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
        let module = open.module;
        self.modules.send(module, vec![Event::Close { replica }]);
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
        let module = open.module;
        let event = Event::Head {
            replica,
            event: HeadEvent::Moved {
                head,
                from_this_device,
            },
        };
        open.replay.push(event.clone());
        self.modules.send(module, vec![event]);
    }

    pub async fn request(
        &mut self,
        ctx: Ctx<'_, H>,
        block: Uuid,
        request: impl FnOnce(ReplicaId) -> Request,
    ) -> Reply {
        let Some((replica, open)) = self.replica_of(block) else {
            return Reply::Unknown;
        };
        let module = open.module;
        self.ask(ctx, module, request(replica)).await
    }

    pub fn request_later(
        &mut self,
        block: Uuid,
        request: impl FnOnce(ReplicaId) -> Request,
    ) -> Option<tokio::sync::oneshot::Receiver<Reply>> {
        let (replica, open) = self.replica_of(block)?;
        let module = open.module;
        self.ask_later(module, request(replica))
    }

    pub async fn content(&mut self, ctx: Ctx<'_, H>, block: Uuid) -> Option<Vec<u8>> {
        match self
            .request(ctx, block, |replica| Request::Content { replica })
            .await
        {
            Reply::Content(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub async fn change(&mut self, ctx: Ctx<'_, H>, block: Uuid, change: Change) -> bool {
        matches!(
            self.request(ctx, block, |replica| Request::Change { replica, change })
                .await,
            Reply::Changed(true)
        )
    }

    pub async fn seal(&mut self, ctx: Ctx<'_, H>, block: Uuid, force: bool) {
        if let Some((replica, open)) = self.replica_of(block)
            && open.joining
            && open.session.owner == Some(JOINING_OWNER)
        {
            if open.status.clean {
                return;
            }
            self.make_up_session(replica);
        }
        self.request(ctx, block, |replica| Request::Seal { replica, force })
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
    ) -> Option<(Vec<u8>, usize)> {
        let module = self.modules.module_of(content_type)?;
        let request = Request::Merge3 {
            content_type,
            base: base.map(<[u8]>::to_vec),
            ours: ours.map(<[u8]>::to_vec),
            theirs: theirs.map(<[u8]>::to_vec),
        };
        match self.ask(ctx, module, request).await {
            Reply::Merged { bytes, conflicts } => Some((bytes, conflicts)),
            _ => None,
        }
    }

    pub async fn merge_diverged(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        block: Uuid,
        local: CommitId,
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
            let Some(theirs) = remote else {
                return Ok(None);
            };
            peer.fetch_history(theirs).await?;
            let be_session::Resume::Merge { base, ours, theirs } =
                be_session::resume(peer.commits(), Some(local), Some(theirs))?
            else {
                return Ok(None);
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
                    return Ok(Some(head));
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
    ) -> Option<block_type_api::Description> {
        let module = self.modules.module_of(content_type)?;
        let request = Request::Describe {
            content_type,
            bytes: bytes.to_vec(),
            organization: ctx.0.organization(),
        };
        match self.ask(ctx, module, request).await {
            Reply::Described(description) => Some(description),
            _ => None,
        }
    }

    pub async fn to_file(
        &mut self,
        ctx: Ctx<'_, H>,
        content_type: Uuid,
        bytes: Vec<u8>,
    ) -> Result<Option<block_type_api::PlainFile>, String> {
        let module = self
            .modules
            .module_of(content_type)
            .ok_or_else(|| format!("no type module has the block type {content_type}"))?;
        let request = Request::ToFile {
            content_type,
            bytes,
        };
        match self.ask(ctx, module, request).await {
            Reply::File(file) => Ok(file),
            Reply::Malformed(error) => Err(error),
            other => Err(format!("the type module answered {other:?}")),
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
        let request = Request::Apply {
            content_type,
            bytes,
            client,
            change,
        };
        match self.ask(ctx, module, request).await {
            Reply::Content(bytes) => Ok(Some(bytes)),
            Reply::Changed(false) => Ok(None),
            Reply::Malformed(error) => Err(error),
            other => Err(format!("the type module answered {other:?}")),
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
        if let Some((replica, open)) = self.replica_of(block) {
            let module = open.module;
            self.modules.send(module, vec![Event::Claim { replica }]);
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
                let event = Event::Lease {
                    replica,
                    session: state.clone(),
                    resynchronize: false,
                };
                if open.joining {
                    return open.early.push(event);
                }
                open.session = state.clone();
                let module = open.module;
                open.replay.push(event.clone());
                self.modules.send(module, vec![event]);
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
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                let module = open.module;
                let event = Event::Message {
                    replica,
                    from: Endpoint::Peer(*from),
                    bytes,
                };
                if open.joining {
                    return open.early.push(event);
                }
                open.replay.push(event.clone());
                self.modules.send(module, vec![event]);
            }
            ServerMessage::HeadChanged { block, head, .. } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                if let Some(open) = self.open.get_mut(&replica)
                    && open.joining
                {
                    return open.early.push(Event::Head {
                        replica,
                        event: HeadEvent::Moved {
                            head: *head,
                            from_this_device: false,
                        },
                    });
                }
                self.moved(replica, *head, false);
            }
            _ => {}
        }
    }

    pub async fn resynchronize(&mut self, (peer, shared): Ctx<'_, H>) {
        for (replica, block) in self
            .open
            .iter()
            .filter(|(_, open)| !open.joining)
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
            let module = open.module;
            self.modules.send(
                module,
                vec![Event::Lease {
                    replica,
                    session,
                    resynchronize: true,
                }],
            );
        }
    }

    pub fn answered(&mut self, io: Io, shared: &Arc<Mutex<H>>) {
        match io {
            Io::Answer {
                replica,
                epoch,
                event,
                published,
            } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                if open.epoch != epoch {
                    return;
                }
                let module = open.module;
                if matches!(*event, Event::Head { .. }) {
                    open.publishing = open.publishing.saturating_sub(1);
                }
                if let Some(head) = published {
                    open.published.push(head);
                }
                self.modules.send(module, vec![*event]);
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
            Io::Sealed => {}
            Io::Joined {
                replica,
                block,
                joined,
            } => self.joined(shared, replica, block, *joined),
        }
    }

    pub fn disconnected(&mut self) {
        let replicas: Vec<ReplicaId> = self.open.keys().copied().collect();
        for replica in replicas {
            let Some(open) = self.open.get_mut(&replica) else {
                continue;
            };
            let joining = std::mem::replace(&mut open.joining, false);
            open.early.clear();
            self.make_up_session(replica);
            if joining {
                self.close_if_asked(replica);
            }
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
            let joining = std::mem::replace(&mut open.joining, false);
            open.made_up = false;
            open.early.clear();
            let known = open.status.head;
            self.reopen_joined(replica, client, session, known);
            if let Some(head) = head
                && Some(head) != known
            {
                self.moved(replica, head, false);
            }
            if joining {
                self.close_if_asked(replica);
            }
        }
    }

    pub async fn restart(&mut self, module: usize) {
        for (replica, open) in self
            .open
            .iter_mut()
            .filter(|(_, open)| open.module == module)
        {
            open.epoch += 1;
            let mut events = vec![Event::Open {
                replica: *replica,
                block: open.block,
                content_type: open.content_type,
                organization: open.organization,
                client: open.client,
                tag: self.modules.tag(module),
                head: open.status.head,
                session: open.session.clone(),
            }];
            for (editor, editable) in &open.editors {
                events.push(Event::Attach {
                    replica: *replica,
                    editor: *editor,
                    editable: *editable,
                });
            }
            events.extend(open.replay.iter().cloned());
            events.extend(asked_of(&self.asked, *replica));
            open.publishing = 0;
            open.moved.clear();
            self.modules.send(module, events);
            if open.closing && !open.joining {
                self.modules
                    .send(module, vec![Event::Close { replica: *replica }]);
            }
        }
    }

    pub async fn act(
        &mut self,
        (peer, shared): Ctx<'_, H>,
        module: usize,
        command: Command,
    ) -> bool {
        let replica_of = |replica: ReplicaId, open: &HashMap<ReplicaId, Open>| {
            open.get(&replica)
                .map(|open| (open.block, open.content_type, open.module == module))
        };
        let made_up = |replica: ReplicaId, open: &HashMap<ReplicaId, Open>| {
            open.get(&replica).is_some_and(|open| open.made_up)
        };
        match command {
            Command::Hello { .. } | Command::WakeAt { .. } => false,
            Command::Reply { id, reply } => {
                self.asked.remove(&id);
                if let Some(answer) = self.replies.remove(&id) {
                    let _ = answer.send(reply);
                    return false;
                }
                let Some(replica) = self.contents.remove(&id) else {
                    return false;
                };
                let (Some(open), Reply::Content(bytes)) = (self.open.get(&replica), reply) else {
                    return false;
                };
                H::content(shared, open.block, open.content_type, bytes);
                true
            }
            Command::Send { replica, to, bytes } => {
                let Some((block, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                let to_peers = !matches!(to, Recipients::One(Endpoint::Editor(_) | Endpoint::Host));
                if to_peers && made_up(replica, &self.open) {
                    return false;
                }
                match to {
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
                }
            }
            Command::Read {
                replica,
                token,
                commit,
                range,
            } => {
                let Some((_, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                H::spawn(async move {
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
                    let _ = io.send(Io::Answer {
                        replica,
                        epoch,
                        event: Box::new(Event::Read {
                            replica,
                            token,
                            result,
                        }),
                        published: None,
                    });
                    H::wake();
                });
                false
            }
            Command::Publish {
                replica,
                token,
                expected,
                merging,
                content,
            } => {
                let Some((block, content_type, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                if let Some(open) = self.open.get_mut(&replica) {
                    open.publishing += 1;
                }
                let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
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
                        move || {
                            let _ = sealed.send(Io::Sealed);
                        },
                    )
                    .await;
                    let (event, published) = match saved {
                        Ok(Saved::Published(head)) => {
                            (HeadEvent::Published { token, head }, Some(head))
                        }
                        Ok(Saved::Unchanged(head)) => (HeadEvent::Published { token, head }, None),
                        Ok(Saved::Rejected { head }) => (HeadEvent::Rejected { token, head }, None),
                        Err(error) => (
                            HeadEvent::Failed {
                                token,
                                error: error.to_string(),
                            },
                            None,
                        ),
                    };
                    let _ = io.send(Io::Answer {
                        replica,
                        epoch,
                        event: Box::new(Event::Head { replica, event }),
                        published,
                    });
                    H::wake();
                });
                false
            }
            Command::Stash {
                replica,
                token,
                parent,
                content,
            } => {
                let Some((block, content_type, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                if let Some(open) = self.open.get_mut(&replica) {
                    open.publishing += 1;
                }
                let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                H::spawn(async move {
                    let stashed = peer
                        .stash_encoded(
                            block,
                            content_type,
                            &content.bytes,
                            content.references,
                            content.derived,
                            parent,
                        )
                        .await;
                    let event = match stashed {
                        Ok(commit) => HeadEvent::Stashed { token, commit },
                        Err(error) => HeadEvent::Failed {
                            token,
                            error: error.to_string(),
                        },
                    };
                    let _ = io.send(Io::Answer {
                        replica,
                        epoch,
                        event: Box::new(Event::Head { replica, event }),
                        published: None,
                    });
                    H::wake();
                });
                false
            }
            Command::Resume {
                replica,
                token,
                local,
            } => {
                let Some((block, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                let epoch = self.open.get(&replica).map_or(0, |open| open.epoch);
                let joining = self.open.get(&replica).is_some_and(|open| open.joining);
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                H::spawn(async move {
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
                    .await;
                    let _ = io.send(Io::Answer {
                        replica,
                        epoch,
                        event: Box::new(Event::Resumed {
                            replica,
                            token,
                            result: resumed.map_err(|error| error.to_string()),
                        }),
                        published: None,
                    });
                    H::wake();
                });
                false
            }
            Command::ClaimLease {
                replica,
                generation,
            } => {
                let Some((block, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                if !peer.is_attached() || made_up(replica, &self.open) {
                    return false;
                }
                match peer.claim_ownership(block, generation).await {
                    Ok(state) => {
                        if let Some(open) = self.open.get_mut(&replica) {
                            open.session = state.clone();
                            let event = Event::Lease {
                                replica,
                                session: state,
                                resynchronize: false,
                            };
                            open.replay.push(event.clone());
                            self.modules.send(module, vec![event]);
                        }
                    }
                    Err(error) if unreachable(&error) => {}
                    Err(error) => H::record(shared, error),
                }
                false
            }
            Command::ReportClean {
                replica,
                generation,
                clean_at,
            } => {
                let Some((block, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                if !peer.is_attached() || made_up(replica, &self.open) {
                    return false;
                }
                if let Err(error) = peer.heartbeat(block, generation, clean_at).await
                    && !unreachable(&error)
                {
                    H::record(shared, error);
                }
                false
            }
            Command::Release { replica } => {
                let Some(open) = self.open.remove(&replica) else {
                    return false;
                };
                self.asked.retain(|_, (to, _)| *to != replica);
                self.blocks.remove(&open.block);
                if !open.joining {
                    let _ = peer.leave_session(open.block).await;
                    let _ = peer.unwatch(open.block).await;
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
            Command::Status { replica, status } => {
                let Some(open) = self.open.get_mut(&replica) else {
                    return false;
                };
                if open.module != module {
                    return false;
                }
                let block = open.block;
                open.unreported = false;
                if status.clean && status.head.is_some() && !status.owes_seal {
                    open.replay.clear();
                }
                let became_unreadable =
                    status.unreadable.is_some() && open.status.unreadable.is_none();
                open.status = status.clone();
                let content_type = open.content_type;
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
                                self.modules.send(module, vec![Event::Close { replica }]);
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
            Command::Described {
                replica,
                description,
            } => {
                let Some(open) = self.open.get(&replica) else {
                    return false;
                };
                let block = open.block;
                let reader = open.reader;
                let references = peer.resolve(block, description.references.clone());
                H::described(shared, block, references, description);
                if reader {
                    let id = self.modules.next_request();
                    self.contents.insert(id, replica);
                    self.modules.send(
                        module,
                        vec![Event::Request {
                            id,
                            request: Request::Content { replica },
                        }],
                    );
                }
                true
            }
            Command::History {
                replica,
                can_undo,
                can_redo,
            } => {
                let Some(open) = self.open.get(&replica) else {
                    return false;
                };
                H::history(shared, open.block, can_undo, can_redo);
                true
            }
            Command::Conflict {
                replica,
                ours,
                theirs,
                count,
            } => {
                let Some(open) = self.open.get(&replica) else {
                    return false;
                };
                H::conflict(
                    shared,
                    open.block,
                    crate::Conflict {
                        ours,
                        theirs,
                        count,
                    },
                );
                true
            }
            Command::Error { replica, message } => {
                let block = self.open.get(&replica).map(|open| open.block);
                let name =
                    block.map_or_else(|| "a type module".to_owned(), |block| block.to_string());
                H::report(shared, block, format!("{name}: {message}"));
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

async fn commit<H: Host>(
    peer: &Peer<H::Store>,
    block: Uuid,
    content_type: Uuid,
    content: block_type_api::Written,
    expected: Option<CommitId>,
    merging: Option<(Option<CommitId>, CommitId)>,
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
    sealed();
    if !peer.is_attached() {
        return Ok(Saved::Unchanged(commit));
    }
    match peer.publish_sealed(block, commit).await {
        Ok(Saved::Rejected { head }) => {
            peer.withdraw(block, commit, before)?;
            Ok(Saved::Rejected { head })
        }
        Err(error) if unreachable(&error) => Ok(Saved::Unchanged(commit)),
        saved => saved,
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
