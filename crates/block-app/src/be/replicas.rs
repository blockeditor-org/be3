use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, PoisonError},
    time::Instant,
};

use be_client::{ClientError, Peer, Saved};
use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::{ClientId, ServerMessage, SessionState};
use block_type_api::{
    ByteRange, Change, Command, EditorId, Endpoint, Event, HeadEvent, Recipients, ReplicaId,
    Reply, Request, RequestId, Status,
};
use block_type_guest::editor::{FromEditor, ToEditor};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use super::{
    Content,
    types::{self, Modules},
    worker::{PeerMessage, Presence, Shared, Store, record},
};

pub(crate) const HOST_EDITOR: EditorId = u64::MAX;

pub(super) enum Io {
    Answer {
        replica: ReplicaId,
        epoch: u64,
        event: Event,
        published: Option<CommitId>,
    },
    Locked {
        replica: ReplicaId,
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
}

pub(super) struct Replicas {
    pub(super) modules: Modules,
    answers: UnboundedReceiver<Io>,
    open: HashMap<ReplicaId, Open>,
    blocks: HashMap<Uuid, ReplicaId>,
    next: ReplicaId,
    io: UnboundedSender<Io>,
    contents: HashMap<RequestId, ReplicaId>,
    replies: HashMap<RequestId, tokio::sync::oneshot::Sender<Reply>>,
    pending: Vec<Io>,
}

pub(super) type Ctx<'a> = (&'a Arc<Peer<Store>>, &'a Arc<Mutex<Shared>>);

impl Replicas {
    pub(super) fn new(modules: Modules) -> Self {
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
        }
    }

    pub(super) async fn woken(&mut self) {
        let notified = std::pin::pin!(self.modules.notified());
        let answered = std::pin::pin!(self.answers.recv());
        if let futures_util::future::Either::Right((Some(io), _)) =
            futures_util::future::select(notified, answered).await
        {
            self.pending.push(io);
        }
    }

    pub(super) async fn pump(&mut self, ctx: Ctx<'_>) -> bool {
        let mut changed = false;
        loop {
            while let Ok(io) = self.answers.try_recv() {
                self.pending.push(io);
            }
            let answers = std::mem::take(&mut self.pending);
            let commands = self.modules.take();
            for module in self.modules.take_restarted() {
                self.restart(module).await;
                changed = true;
            }
            if answers.is_empty() && commands.is_empty() {
                return changed;
            }
            for io in answers {
                self.answered(io, ctx.1);
            }
            for (module, command) in commands {
                changed |= self.act(ctx, module, command).await;
            }
        }
    }

    pub(super) async fn ask(&mut self, ctx: Ctx<'_>, module: usize, request: Request) -> Reply {
        let Some(answered) = self.ask_later(module, request) else {
            return Reply::Unknown;
        };
        let mut answered = std::pin::pin!(answered);
        loop {
            self.pump(ctx).await;
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
        self.modules
            .send(module, vec![Event::Request { id, request }]);
        Some(answered)
    }

    pub(super) fn is_open(&self, block: Uuid) -> bool {
        self.blocks.contains_key(&block)
    }

    pub(super) fn open_blocks(&self) -> Vec<Uuid> {
        self.blocks.keys().copied().collect()
    }

    fn replica_of(&self, block: Uuid) -> Option<(ReplicaId, &Open)> {
        let replica = *self.blocks.get(&block)?;
        Some((replica, self.open.get(&replica)?))
    }

    pub(super) async fn open(
        &mut self,
        (peer, shared): Ctx<'_>,
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
        let owned = !kind.frozen
            && shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .owned
                .contains(&block);
        let joined = async {
            peer.ensure_type(block, content_type, BlockParent::Detached)
                .await?;
            if owned && reshaped(shared, block, &kind.schema) {
                reset(peer, block, &kind).await?;
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
                super::worker::set_locked(shared, block, true);
                return;
            }
            Err(error) => {
                record(shared, error);
                return;
            }
        };
        if self.blocks.contains_key(&block) {
            return;
        }
        self.next += 1;
        let replica = self.next;
        let module = kind.module;
        let organization = peer.organization();
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
        let held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        let current = held.graph.get(block).and_then(|node| node.head);
        drop(held);
        if let Some(current) = current
            && Some(current) != head
        {
            self.moved(replica, current, false);
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

    pub(super) fn attach(&mut self, block: Uuid, editor: EditorId, editable: bool) {
        let Some((replica, open)) = self.replica_of(block) else {
            return;
        };
        if open.editors.get(&editor) == Some(&editable) {
            return;
        }
        let module = open.module;
        self.attach_now(module, replica, editor, editable);
    }

    pub(super) fn detach(&mut self, block: Uuid, editor: EditorId) -> bool {
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

    pub(super) fn from_editor(&mut self, block: Uuid, editor: EditorId, bytes: Vec<u8>, at: Instant) {
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
        self.modules.tick_at(module, at);
        self.modules.send(module, vec![event]);
    }

    pub(super) fn from_host(&mut self, block: Uuid, message: &FromEditor, at: Instant) {
        if let Some((replica, open)) = self.replica_of(block)
            && !open.editors.contains_key(&HOST_EDITOR)
        {
            let module = open.module;
            self.attach_now(module, replica, HOST_EDITOR, true);
        }
        self.from_editor(block, HOST_EDITOR, message.encode(), at);
    }

    pub(super) fn close(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).copied() else {
            return;
        };
        let Some(open) = self.open.get_mut(&replica) else {
            return;
        };
        if std::mem::replace(&mut open.closing, true) {
            return;
        }
        let module = open.module;
        self.modules.send(module, vec![Event::Close { replica }]);
    }

    pub(super) fn moved_here(&mut self, block: Uuid, head: CommitId) {
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

    pub(super) async fn request(
        &mut self,
        ctx: Ctx<'_>,
        block: Uuid,
        request: impl FnOnce(ReplicaId) -> Request,
    ) -> Reply {
        let Some((replica, open)) = self.replica_of(block) else {
            return Reply::Unknown;
        };
        let module = open.module;
        self.ask(ctx, module, request(replica)).await
    }

    pub(super) fn request_later(
        &mut self,
        block: Uuid,
        request: impl FnOnce(ReplicaId) -> Request,
    ) -> Option<tokio::sync::oneshot::Receiver<Reply>> {
        let (replica, open) = self.replica_of(block)?;
        let module = open.module;
        self.ask_later(module, request(replica))
    }

    pub(super) async fn content(&mut self, ctx: Ctx<'_>, block: Uuid) -> Option<Vec<u8>> {
        match self
            .request(ctx, block, |replica| Request::Content { replica })
            .await
        {
            Reply::Content(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub(super) async fn change(&mut self, ctx: Ctx<'_>, block: Uuid, change: Change) -> bool {
        matches!(
            self.request(ctx, block, |replica| Request::Change { replica, change })
                .await,
            Reply::Changed(true)
        )
    }

    pub(super) async fn seal(&mut self, ctx: Ctx<'_>, block: Uuid, force: bool) {
        self.request(ctx, block, |replica| Request::Seal { replica, force })
            .await;
    }

    pub(super) async fn seal_all(&mut self, ctx: Ctx<'_>) {
        for block in self.open_blocks() {
            self.seal(ctx, block, false).await;
        }
    }

    pub(super) async fn merge(
        &mut self,
        ctx: Ctx<'_>,
        content_type: Uuid,
        base: Option<&[u8]>,
        ours: Option<&[u8]>,
        theirs: Option<&[u8]>,
    ) -> Option<(Vec<u8>, bool)> {
        let kind = types::kind_of(content_type)?;
        let request = Request::Merge3 {
            content_type,
            base: base.map(<[u8]>::to_vec),
            ours: ours.map(<[u8]>::to_vec),
            theirs: theirs.map(<[u8]>::to_vec),
        };
        match self.ask(ctx, kind.module, request).await {
            Reply::Merged { bytes, conflicts } => Some((bytes, conflicts > 0)),
            _ => None,
        }
    }

    pub(super) async fn describe(
        &mut self,
        ctx: Ctx<'_>,
        content_type: Uuid,
        bytes: &[u8],
    ) -> Option<block_type_api::Description> {
        let kind = types::kind_of(content_type)?;
        let request = Request::Describe {
            content_type,
            bytes: bytes.to_vec(),
            organization: ctx.0.organization(),
        };
        match self.ask(ctx, kind.module, request).await {
            Reply::Described(description) => Some(description),
            _ => None,
        }
    }

    pub(super) fn status(&self, block: Uuid) -> Option<Status> {
        self.replica_of(block).map(|(_, open)| open.status.clone())
    }

    pub(super) async fn heard(&mut self, (peer, shared): Ctx<'_>, event: &ServerMessage) {
        match event {
            ServerMessage::SessionChanged { block, state } => {
                let Some(replica) = self.blocks.get(block).copied() else {
                    return;
                };
                let Some(open) = self.open.get_mut(&replica) else {
                    return;
                };
                open.session = state.clone();
                let module = open.module;
                let event = Event::Lease {
                    replica,
                    session: state.clone(),
                    resynchronize: false,
                };
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
                    Err(error) => return record(shared, error),
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
                open.replay.push(event.clone());
                self.modules.send(module, vec![event]);
            }
            ServerMessage::HeadChanged { block, head, .. } => {
                if let Some(replica) = self.blocks.get(block).copied() {
                    self.moved(replica, *head, false);
                }
            }
            _ => {}
        }
    }

    pub(super) async fn resynchronize(&mut self, (peer, shared): Ctx<'_>) {
        for (replica, block) in self
            .open
            .iter()
            .map(|(replica, open)| (*replica, open.block))
            .collect::<Vec<_>>()
        {
            let session = match peer.join_session(block).await {
                Ok((_, session)) => session,
                Err(error) => {
                    record(shared, error);
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

    pub(super) fn answered(&mut self, io: Io, shared: &Arc<Mutex<Shared>>) {
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
                if matches!(event, Event::Head { .. }) {
                    open.publishing = open.publishing.saturating_sub(1);
                }
                if let Some(head) = published {
                    open.published.push(head);
                }
                self.modules.send(module, vec![event]);
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
                    super::worker::set_locked(shared, open.block, true);
                }
            }
        }
    }

    pub(super) async fn reconnect(&mut self, (peer, shared): Ctx<'_>) {
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
                    record(shared, error);
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
            open.replay.retain(|event| {
                matches!(
                    event,
                    Event::Message {
                        from: Endpoint::Editor(_),
                        ..
                    }
                )
            });
            let module = open.module;
            let mut events = vec![Event::Open {
                replica,
                block,
                content_type: open.content_type,
                organization: open.organization,
                client,
                tag: self.modules.tag(module),
                head: open.status.head,
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
            let known = open.status.head;
            self.modules.send(module, events);
            if let Some(head) = head
                && Some(head) != known
            {
                self.moved(replica, head, false);
            }
        }
    }

    pub(super) async fn restart(&mut self, module: usize) {
        for (replica, open) in self.open.iter_mut().filter(|(_, open)| open.module == module) {
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
            open.publishing = 0;
            open.moved.clear();
            self.modules.send(module, events);
            if open.closing {
                self.modules.send(module, vec![Event::Close { replica: *replica }]);
            }
        }
    }

    pub(super) async fn act(
        &mut self,
        (peer, shared): Ctx<'_>,
        module: usize,
        command: Command,
    ) -> bool {
        let replica_of = |replica: ReplicaId, open: &HashMap<ReplicaId, Open>| {
            open.get(&replica)
                .map(|open| (open.block, open.content_type, open.module == module))
        };
        match command {
            Command::Hello { .. } | Command::WakeAt { .. } => false,
            Command::Reply { id, reply } => {
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
                let block = open.block;
                let content_type = open.content_type;
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                let revision = held.blocks.get(&block).map_or(0, |content| content.revision);
                let mut content = Content::new(content_type, bytes);
                content.revision = revision + 1;
                held.blocks.insert(block, content);
                held.previewed.remove(&block);
                held.touch(block);
                true
            }
            Command::Send {
                replica,
                to,
                bytes,
            } => {
                let Some((block, _, true)) = replica_of(replica, &self.open) else {
                    return false;
                };
                match to {
                    Recipients::One(Endpoint::Editor(editor)) => {
                        to_editor(shared, block, editor, bytes);
                        true
                    }
                    Recipients::One(Endpoint::Host) => false,
                    Recipients::One(Endpoint::Peer(client)) => {
                        if let Err(error) = peer.relay(block, Some(client), &bytes).await {
                            record(shared, error);
                        }
                        false
                    }
                    Recipients::Peers => {
                        if let Err(error) = peer.relay(block, None, &bytes).await {
                            record(shared, error);
                        }
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
                super::platform::spawn_local(async move {
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
                        event: Event::Read {
                            replica,
                            token,
                            result,
                        },
                        published: None,
                    });
                    crate::host::wake();
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
                super::platform::spawn_local(async move {
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
                        event: Event::Head { replica, event },
                        published,
                    });
                    crate::host::wake();
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
                super::platform::spawn_local(async move {
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
                        event: Event::Head { replica, event },
                        published: None,
                    });
                    crate::host::wake();
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
                let peer = Arc::clone(peer);
                let io = self.io.clone();
                super::platform::spawn_local(async move {
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
                    .await;
                    let _ = io.send(Io::Answer {
                        replica,
                        epoch,
                        event: Event::Resumed {
                            replica,
                            token,
                            result: resumed.map_err(|error| error.to_string()),
                        },
                        published: None,
                    });
                    crate::host::wake();
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
                    Err(error) => record(shared, error),
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
                if let Err(error) = peer.heartbeat(block, generation, clean_at).await {
                    record(shared, error);
                }
                false
            }
            Command::Release { replica } => {
                let Some(open) = self.open.remove(&replica) else {
                    return false;
                };
                self.blocks.remove(&open.block);
                let _ = peer.leave_session(open.block).await;
                let _ = peer.unwatch(open.block).await;
                if open.reopen {
                    self.open(
                        (peer, shared),
                        open.block,
                        open.content_type,
                        open.reader,
                    )
                    .await;
                    return true;
                }
                let block = open.block;
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.blocks.remove(&block);
                held.described.remove(&block);
                held.graph.forget_derived(block);
                held.graph.forget_references(block);
                held.previewed.remove(&block);
                held.histories.remove(&block);
                held.presence.remove(&block);
                held.peer_messages.remove(&block);
                held.touch(block);
                drop(held);
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
                if status.clean && status.head.is_some() && !status.owes_seal {
                    open.replay.clear();
                }
                let unreadable = status.unreadable.clone();
                let became_unreadable = unreadable.is_some() && open.status.unreadable.is_none();
                open.status = status.clone();
                let content_type = open.content_type;
                if became_unreadable {
                    let owned = shared
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .owned
                        .contains(&block);
                    if let Some(kind) = types::kind_of(content_type)
                        && owned
                        && !kind.frozen
                    {
                        match reset(peer, block, &kind).await {
                            Ok(()) => {
                                if let Some(open) = self.open.get_mut(&replica) {
                                    open.reopen = true;
                                    open.closing = true;
                                }
                                self.modules.send(module, vec![Event::Close { replica }]);
                                return true;
                            }
                            Err(error) => record(shared, error),
                        }
                    }
                }
                let locked = {
                    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                    let before = held.graph.get(block).and_then(|node| node.head);
                    if before != status.head {
                        held.touch(block);
                    }
                    held.graph.set_head(block, status.head);
                    held.locked.contains(&block)
                };
                if !locked || unreadable.is_none() {
                    super::worker::set_unreadable(shared, block, unreadable);
                }
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
                {
                    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                    held.graph.refer(block, references);
                    held.described.insert(block, description);
                    held.touch(block);
                }
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
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.histories.insert(
                    open.block,
                    super::History { can_undo, can_redo },
                );
                held.touch(open.block);
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
                let block = open.block;
                let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
                held.error = Some(format!(
                    "block {block} merged with {count} conflicting change(s); both versions are kept in its history"
                ));
                held.conflicts
                    .entry(block)
                    .or_default()
                    .push(be_client::Conflict {
                        ours,
                        theirs,
                        count,
                    });
                true
            }
            Command::Error { replica, message } => {
                let name = self
                    .open
                    .get(&replica)
                    .map(|open| open.block.to_string())
                    .unwrap_or_else(|| "a type module".to_owned());
                shared.lock().unwrap_or_else(PoisonError::into_inner).error =
                    Some(format!("{name}: {message}"));
                false
            }
        }
    }

    fn counted(&self, shared: &Arc<Mutex<Shared>>) {
        let unsealed = self
            .open
            .values()
            .filter(|open| !open.status.clean)
            .count();
        let owing = self
            .open
            .values()
            .filter(|open| open.status.owes_seal)
            .count();
        let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.unsealed = unsealed;
        held.owing = owing;
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

fn reshaped(shared: &Arc<Mutex<Shared>>, block: Uuid, schema: &str) -> bool {
    shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .graph
        .get(block)
        .and_then(|node| node.metadata.get(&be_block::SCHEMA))
        .is_some_and(|stamped| stamped != schema)
}

async fn reset(peer: &Peer<Store>, block: Uuid, kind: &types::Kind) -> Result<(), ClientError> {
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
    crate::notices::report(
        "Some of this device's settings were saved by another build of Block, so they were reset."
            .to_owned(),
    );
    Ok(())
}

fn to_editor(shared: &Arc<Mutex<Shared>>, block: Uuid, editor: EditorId, bytes: Vec<u8>) {
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    if editor == HOST_EDITOR {
        match ToEditor::decode(&bytes) {
            Some(ToEditor::Presence(peers)) => {
                held.presence_revision += 1;
                let revision = held.presence_revision;
                held.presence.insert(
                    block,
                    (
                        revision,
                        peers
                            .into_iter()
                            .map(|peer| Presence {
                                client: peer.client,
                                kind: peer.kind,
                                value: peer.value,
                            })
                            .collect(),
                    ),
                );
                held.touch(block);
            }
            Some(ToEditor::PeerMessages(messages)) => {
                for message in messages {
                    held.peer_message_count += 1;
                    let sequence = held.peer_message_count;
                    let queue = held.peer_messages.entry(block).or_default();
                    queue.push_back((
                        sequence,
                        PeerMessage {
                            from: message.from,
                            channel: message.channel,
                            payload: message.payload,
                        },
                    ));
                    if queue.len() > super::worker::PEER_MESSAGE_LIMIT {
                        queue.pop_front();
                    }
                }
                held.touch(block);
            }
            _ => {}
        }
        return;
    }
    held.editor_frames
        .entry(editor)
        .or_default()
        .push((block, frame(bytes)));
    drop(held);
    crate::host::wake();
}

fn frame(mut bytes: Vec<u8>) -> Vec<u8> {
    let mut framed = (bytes.len() as u32).to_be_bytes().to_vec();
    framed.append(&mut bytes);
    framed
}

pub(crate) fn unframe(frame: &[u8]) -> Option<Vec<u8>> {
    let (length, payload) = frame.split_first_chunk::<4>()?;
    (u32::from_be_bytes(*length) as usize == payload.len()).then(|| payload.to_vec())
}

pub(crate) fn snapshot_frame(content_type: Uuid, bytes: Vec<u8>) -> Vec<u8> {
    frame(
        ToEditor::Snapshot(block_type_guest::editor::Snapshot {
            content_type,
            bytes,
            session: Vec::new(),
            unsettled: Vec::new(),
            applied: 0,
        })
        .encode(),
    )
}
