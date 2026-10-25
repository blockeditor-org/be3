use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::rc::Rc;

use be_store::Hash;
use block_editor_beui::be_block::{LiveEdit, Merge, Undo};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, SeededContent};
use block_plugin_api::{BlockCommand, EditorInstanceId, EditorMessage, Message, StoredHead};
use block_type_guest::api::{
    Change, ClientId, Command, CommitId, EditorId, Endpoint, Event, HeadEvent, Recipients,
    ReplicaId, Reply, Request, RequestId, Resume, SessionState, SessionTag,
};
use block_type_guest::editor::{EditorMirror, FromEditor, Gesture, ToEditor};
use block_type_guest::wire::Wire;
use block_type_guest::{Module, Registration};
use uuid::Uuid;

const THIS_DEVICE: ClientId = u64::MAX - 1;
const ELSEWHERE: ClientId = u64::MAX;
const TAG: SessionTag = SessionTag([0; 32]);
const STORE: EditorId = u64::MAX;

fn every_type() -> Vec<Registration> {
    host_types::registrations()
        .into_iter()
        .chain([Registration::of::<workspace_ui_types::EditorViewContent>()])
        .map(Registration::without_undo)
        .collect()
}

struct Block {
    content_type: Uuid,
    head: Option<CommitId>,
    replica: Option<ReplicaId>,
    written: Vec<Vec<u8>>,
    unapplied: Vec<Vec<u8>>,
    followed: Vec<ToEditor>,
    references: Option<Vec<Uuid>>,
    revision: u64,
    taken: u64,
}

impl Block {
    fn new(content_type: Uuid, head: CommitId) -> Self {
        Self {
            content_type,
            head: Some(head),
            replica: None,
            written: Vec::new(),
            unapplied: Vec::new(),
            followed: Vec::new(),
            references: None,
            revision: 1,
            taken: 0,
        }
    }
}

struct Inner {
    own: Option<Uuid>,
    organization: Uuid,
    module: Module,
    now: u64,
    replicas: BTreeMap<ReplicaId, Uuid>,
    next_replica: ReplicaId,
    commits: BTreeMap<CommitId, Vec<u8>>,
    next_commit: u64,
    blocks: BTreeMap<Uuid, Block>,
    attached: BTreeMap<Uuid, BTreeSet<EditorId>>,
    presence: BTreeMap<Uuid, BTreeMap<(ClientId, Uuid), Vec<u8>>>,
    outbox: Vec<(Uuid, EditorId, Vec<u8>)>,
    replies: BTreeMap<RequestId, Reply>,
    next_request: RequestId,
    elsewhere: u64,
    reverted: Option<Vec<Vec<u8>>>,
    seeded: Vec<SeededContent>,
    graph: BTreeMap<Uuid, BlockInfo>,
    created: Vec<Uuid>,
    watched_content: BTreeSet<Uuid>,
    watched_heads: BTreeSet<Uuid>,
    heads_sent: BTreeMap<Uuid, u64>,
    watched_queries: Vec<BlockQuery>,
    answered: BTreeMap<BlockQuery, Vec<BlockInfo>>,
    deferred: Option<Vec<(Uuid, EditorId, FromEditor)>>,
    known_elsewhere: Option<Rc<dyn Fn(Uuid) -> bool>>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            own: None,
            organization: Uuid::nil(),
            module: Module::new(every_type()),
            now: 0,
            replicas: BTreeMap::new(),
            next_replica: 1,
            commits: BTreeMap::new(),
            next_commit: 0,
            blocks: BTreeMap::new(),
            attached: BTreeMap::new(),
            presence: BTreeMap::new(),
            outbox: Vec::new(),
            replies: BTreeMap::new(),
            next_request: 0,
            elsewhere: 0,
            reverted: None,
            seeded: Vec::new(),
            graph: BTreeMap::new(),
            created: Vec::new(),
            watched_content: BTreeSet::new(),
            watched_heads: BTreeSet::new(),
            heads_sent: BTreeMap::new(),
            watched_queries: Vec::new(),
            answered: BTreeMap::new(),
            deferred: None,
            known_elsewhere: None,
        }
    }
}

fn session() -> SessionState {
    SessionState {
        generation: 1,
        owner: Some(THIS_DEVICE),
        participants: vec![THIS_DEVICE],
        clean_at: None,
    }
}

impl Inner {
    fn write(&mut self, bytes: Vec<u8>) -> CommitId {
        self.next_commit += 1;
        let mut id = [0; 32];
        id[..8].copy_from_slice(&self.next_commit.to_be_bytes());
        let commit = CommitId::from_hash(Hash::from_bytes(id));
        self.commits.insert(commit, bytes);
        commit
    }

    fn run(&mut self, event: Event) {
        let mut queue = VecDeque::from([event]);
        let mut handled = 0;
        while let Some(event) = queue.pop_front() {
            handled += 1;
            assert!(handled < 100_000, "the block's replica never settled");
            for command in self.module.handle(event) {
                self.act(command, &mut queue);
            }
        }
    }

    fn act(&mut self, command: Command, queue: &mut VecDeque<Event>) {
        match command {
            Command::Send {
                replica,
                to: Recipients::One(Endpoint::Editor(editor)),
                bytes,
            } => {
                let Some(&block) = self.replicas.get(&replica) else {
                    return;
                };
                if editor != STORE {
                    return self.outbox.push((block, editor, bytes));
                }
                let Some(message) = ToEditor::decode(&bytes) else {
                    return;
                };
                if let (Some(reverted), ToEditor::Operations(operations)) =
                    (&mut self.reverted, &message)
                {
                    reverted.extend(
                        operations
                            .iter()
                            .filter(|operation| !operation.mine)
                            .map(|operation| operation.operation.clone()),
                    );
                }
                if let Some(held) = self.blocks.get_mut(&block) {
                    if matches!(message, ToEditor::Snapshot(_)) {
                        held.followed.clear();
                    }
                    held.followed.push(message);
                }
            }
            Command::Send { .. } => {}
            Command::Publish {
                replica,
                token,
                expected,
                content,
                ..
            } => {
                let Some(&block) = self.replicas.get(&replica) else {
                    return;
                };
                let head = self.blocks.get(&block).and_then(|held| held.head);
                if expected != head {
                    return queue.push_back(Event::Head {
                        replica,
                        event: HeadEvent::Rejected { token, head },
                    });
                }
                let unchanged = head.is_some_and(|head| self.commits[&head] == content.bytes);
                let head = match (unchanged, head) {
                    (true, Some(head)) => head,
                    _ => self.write(content.bytes),
                };
                if let Some(held) = self.blocks.get_mut(&block) {
                    held.head = Some(head);
                }
                queue.push_back(Event::Head {
                    replica,
                    event: HeadEvent::Published { token, head },
                });
            }
            Command::Stash {
                replica,
                token,
                content,
                ..
            } => {
                let commit = self.write(content.bytes);
                queue.push_back(Event::Head {
                    replica,
                    event: HeadEvent::Stashed { token, commit },
                });
            }
            Command::Read {
                replica,
                token,
                commit,
                ..
            } => queue.push_back(Event::Read {
                replica,
                token,
                result: self
                    .commits
                    .get(&commit)
                    .cloned()
                    .ok_or_else(|| "no such commit".to_owned()),
            }),
            Command::Resume {
                replica,
                token,
                local,
                ..
            } => {
                let head = self
                    .replicas
                    .get(&replica)
                    .and_then(|block| self.blocks.get(block))
                    .and_then(|held| held.head);
                let resume = match (local, head) {
                    (None, None) => Resume::Empty,
                    (Some(local), Some(head)) if local == head => Resume::UpToDate,
                    (_, Some(head)) => Resume::FastForward { to: head },
                    (Some(local), None) => Resume::Publish { from: local },
                };
                queue.push_back(Event::Resumed {
                    replica,
                    token,
                    result: Ok(resume),
                });
            }
            Command::Described {
                replica,
                description,
            } => {
                if let Some(held) = self
                    .replicas
                    .get(&replica)
                    .and_then(|block| self.blocks.get_mut(block))
                {
                    held.references = Some(description.references);
                }
            }
            Command::Reply { id, reply } => {
                self.replies.insert(id, reply);
            }
            Command::Release { replica } => {
                if let Some(block) = self.replicas.remove(&replica)
                    && let Some(held) = self.blocks.get_mut(&block)
                    && held.replica == Some(replica)
                {
                    held.replica = None;
                }
            }
            Command::Hello { .. }
            | Command::ClaimLease { .. }
            | Command::ReportClean { .. }
            | Command::WakeAt { .. }
            | Command::Status { .. }
            | Command::History { .. }
            | Command::Conflict { .. }
            | Command::Error { .. } => {}
        }
    }

    fn request(&mut self, request: Request) -> Reply {
        self.next_request += 1;
        let id = self.next_request;
        self.run(Event::Request { id, request });
        self.replies.remove(&id).unwrap_or(Reply::Unknown)
    }

    fn open(&mut self, block: Uuid) {
        let organization = self.organization;
        let Some(held) = self.blocks.get_mut(&block) else {
            return;
        };
        if held.replica.is_some() || !self.module.knows(held.content_type) {
            return;
        }
        let replica = self.next_replica;
        self.next_replica += 1;
        held.replica = Some(replica);
        let (content_type, head) = (held.content_type, held.head);
        let unapplied = std::mem::take(&mut held.unapplied);
        held.followed.clear();
        self.replicas.insert(replica, block);
        self.run(Event::Open {
            replica,
            block,
            content_type,
            organization,
            client: THIS_DEVICE,
            tag: TAG,
            head,
            session: session(),
        });
        self.run(Event::Attach {
            replica,
            editor: STORE,
            editable: false,
        });
        let presence: Vec<((ClientId, Uuid), Vec<u8>)> = self
            .presence
            .get(&block)
            .map(|held| {
                held.iter()
                    .map(|(key, value)| (*key, value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for ((client, kind), value) in presence {
            self.peer_said(
                replica,
                client,
                be_session::SessionMessage::Presence {
                    kind,
                    value: Some(value),
                },
            );
        }
        let editors: Vec<EditorId> = self
            .attached
            .get(&block)
            .map(|held| held.iter().copied().collect())
            .unwrap_or_default();
        for editor in editors {
            self.run(Event::Attach {
                replica,
                editor,
                editable: true,
            });
        }
        for operation in unapplied {
            self.elsewhere(block, operation);
        }
    }

    fn close(&mut self, block: Uuid) {
        let Some(replica) = self.blocks.get(&block).and_then(|held| held.replica) else {
            return;
        };
        self.run(Event::Close { replica });
        self.replicas.remove(&replica);
        if let Some(held) = self.blocks.get_mut(&block) {
            held.replica = None;
        }
    }

    fn peer_said(
        &mut self,
        replica: ReplicaId,
        client: ClientId,
        message: be_session::SessionMessage,
    ) {
        self.run(Event::Message {
            replica,
            from: Endpoint::Peer(client),
            bytes: Wire::Session(message).encode(),
        });
    }

    fn elsewhere(&mut self, block: Uuid, operation: Vec<u8>) {
        let Some(held) = self.blocks.get_mut(&block) else {
            return;
        };
        held.revision += 1;
        let Some(replica) = held.replica else {
            held.unapplied.push(operation);
            return;
        };
        self.elsewhere += 1;
        let id = be_session::OpId {
            client: ELSEWHERE,
            counter: self.elsewhere,
        };
        self.peer_said(
            replica,
            ELSEWHERE,
            be_session::SessionMessage::Submit {
                id,
                base: u64::MAX,
                payload: operation,
            },
        );
    }

    fn reverting(&mut self, block: Uuid, run: impl FnOnce(&mut Self)) {
        self.reverted = Some(Vec::new());
        run(self);
        let reverted = self.reverted.take().unwrap_or_default();
        if let Some(held) = self.blocks.get_mut(&block) {
            held.revision += reverted.len() as u64;
            held.written.extend(reverted);
        }
    }

    fn change(&mut self, block: Uuid, change: Change) {
        if let Some(replica) = self.blocks.get(&block).and_then(|held| held.replica) {
            self.request(Request::Change { replica, change });
        }
    }

    fn put(&mut self, block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
        let same_type = self
            .blocks
            .get(&block)
            .is_some_and(|held| held.content_type == content_type && held.replica.is_some());
        if same_type {
            if let Some(held) = self.blocks.get_mut(&block) {
                held.revision += 1;
            }
            return self.change(block, Change::Replace(bytes));
        }
        self.close(block);
        let head = self.write(bytes);
        match self.blocks.get_mut(&block) {
            Some(held) => {
                held.content_type = content_type;
                held.head = Some(head);
                held.unapplied.clear();
                held.references = None;
                held.revision += 1;
            }
            None => {
                self.blocks.insert(block, Block::new(content_type, head));
            }
        }
        self.open(block);
    }

    fn attach(&mut self, block: Uuid, editor: EditorId) -> bool {
        if !self.attached.entry(block).or_default().insert(editor) {
            return false;
        }
        if let Some(replica) = self.blocks.get(&block).and_then(|held| held.replica) {
            self.run(Event::Attach {
                replica,
                editor,
                editable: true,
            });
        }
        true
    }

    fn detach(&mut self, block: Uuid, editor: EditorId) {
        let removed = self
            .attached
            .get_mut(&block)
            .is_some_and(|held| held.remove(&editor));
        if let (true, Some(replica)) = (
            removed,
            self.blocks.get(&block).and_then(|held| held.replica),
        ) {
            self.run(Event::Detach { replica, editor });
        }
    }

    fn editor_sent(&mut self, block: Uuid, editor: EditorId, message: FromEditor) {
        let frame = message.encode();
        let attached = self.attach(block, editor);
        if attached && message == FromEditor::Resend {
            return;
        }
        if let (FromEditor::Operate(_), Some(deferred)) = (&message, &mut self.deferred) {
            deferred.push((block, editor, message));
            return;
        }
        if let FromEditor::Operate(operation) = &message {
            let Some(held) = self.blocks.get_mut(&block) else {
                return;
            };
            if held.replica.is_none() {
                return;
            }
            held.written.push(operation.clone());
            held.taken += 1;
            held.revision += 1;
        }
        let Some(replica) = self.blocks.get(&block).and_then(|held| held.replica) else {
            return;
        };
        let event = Event::Message {
            replica,
            from: Endpoint::Editor(editor),
            bytes: frame,
        };
        match message {
            FromEditor::Gesture(Gesture::Cancel) => {
                self.reverting(block, |inner| inner.run(event));
            }
            _ => self.run(event),
        }
    }
}

#[derive(Clone, Default)]
pub struct ContentStore(Rc<RefCell<Inner>>);

impl ContentStore {
    pub(crate) fn new(organization: Uuid) -> Self {
        let store = Self::default();
        store.0.borrow_mut().organization = organization;
        store
    }

    pub fn own(&self, block: Uuid, block_type: Uuid) {
        let mut inner = self.0.borrow_mut();
        inner.own = Some(block);
        inner.graph.entry(block).or_insert_with(|| {
            let mut info = BlockInfo::new(block, block_type, BlockParent::Root);
            info.parent = None;
            info
        });
    }

    pub fn add_block(&self, info: BlockInfo) {
        self.0.borrow_mut().graph.insert(info.id, info);
    }

    pub fn block(&self, id: Uuid) -> Option<BlockInfo> {
        self.0.borrow().graph.get(&id).cloned()
    }

    pub fn created(&self) -> Vec<Uuid> {
        self.0.borrow().created.clone()
    }

    pub fn watches(&self, block: Uuid) -> bool {
        self.0.borrow().watched_content.contains(&block)
    }

    fn key(&self, block: Option<Uuid>) -> Uuid {
        block
            .or(self.0.borrow().own)
            .expect("this editor has no block of its own; name the block")
    }

    pub fn register(&self, registrations: impl IntoIterator<Item = Registration>) {
        let mut inner = self.0.borrow_mut();
        for registration in registrations {
            inner.module.register(registration.without_undo());
        }
    }

    fn know<C: LiveEdit + Merge + Clone + Default>(&self) {
        let mut inner = self.0.borrow_mut();
        if !inner.module.knows(C::CONTENT_TYPE) {
            inner.module.register(Registration::of::<C>());
        }
    }

    fn typed<C: LiveEdit + Merge + Clone + Default>(&self, key: Uuid) {
        self.know::<C>();
        let mut inner = self.0.borrow_mut();
        let mismatched = inner
            .blocks
            .get(&key)
            .is_some_and(|held| held.content_type != C::CONTENT_TYPE);
        assert!(
            !mismatched,
            "the store holds that block with another content type"
        );
        inner.open(key);
    }

    pub fn tick(&self, now: u64) {
        let mut inner = self.0.borrow_mut();
        if now > inner.now {
            inner.now = now;
            inner.run(Event::Tick { now });
        }
    }

    pub fn hold<C: LiveEdit + Merge + Clone + Default>(&self, block: Option<Uuid>, content: C) {
        let key = self.key(block);
        self.know::<C>();
        self.0
            .borrow_mut()
            .put(key, C::CONTENT_TYPE, content.encode());
    }

    pub fn keep_history<C: Undo + Merge + Clone + Default + 'static>(&self, block: Option<Uuid>) {
        let key = self.key(block);
        self.typed::<C>(key);
        let mut inner = self.0.borrow_mut();
        assert!(
            inner.blocks.contains_key(&key),
            "the store holds that block"
        );
        inner.module.register(Registration::of::<C>().undo::<C>());
        inner.close(key);
        inner.open(key);
    }

    pub fn content<C: LiveEdit + Merge + Clone + Default>(&self, block: Option<Uuid>) -> C {
        let key = self.key(block);
        self.typed::<C>(key);
        let inner = self.0.borrow();
        let held = inner.blocks.get(&key).expect("the store holds that block");
        let mut mirror = EditorMirror::<C>::default();
        for message in &held.followed {
            mirror.receive(message.clone());
        }
        mirror
            .content()
            .expect("the block's replica has sent its content")
            .clone()
    }

    pub fn applied(&self, block: Option<Uuid>) -> u64 {
        let key = self.key(block);
        self.0
            .borrow()
            .blocks
            .get(&key)
            .map_or(0, |held| held.taken)
    }

    pub fn holds(&self, block: Option<Uuid>) -> bool {
        let key = self.key(block);
        self.0.borrow().blocks.contains_key(&key)
    }

    pub fn edit<C: LiveEdit + Merge + Clone + Default>(
        &self,
        block: Option<Uuid>,
        operation: &C::Op,
    ) {
        let key = self.key(block);
        self.typed::<C>(key);
        assert!(self.holds(Some(key)), "the store holds that block");
        self.0
            .borrow_mut()
            .elsewhere(key, C::encode_operation(operation));
    }

    pub fn take_written(&self, block: Option<Uuid>) -> Vec<Vec<u8>> {
        let key = self.key(block);
        self.0
            .borrow_mut()
            .blocks
            .get_mut(&key)
            .map(|held| std::mem::take(&mut held.written))
            .unwrap_or_default()
    }

    pub fn apply_elsewhere(&self, block: Option<Uuid>, operation: Vec<u8>) {
        let key = self.key(block);
        self.0.borrow_mut().elsewhere(key, operation);
    }

    pub fn set_presence(&self, block: Option<Uuid>, peers: Vec<(ClientId, Uuid, Vec<u8>)>) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let next: BTreeMap<(ClientId, Uuid), Vec<u8>> = peers
            .into_iter()
            .map(|(client, kind, value)| ((client, kind), value))
            .collect();
        let previous = inner.presence.insert(key, next.clone()).unwrap_or_default();
        let Some(replica) = inner.blocks.get(&key).and_then(|held| held.replica) else {
            return;
        };
        for (client, kind) in previous.keys() {
            if !next.contains_key(&(*client, *kind)) {
                inner.peer_said(
                    replica,
                    *client,
                    be_session::SessionMessage::Presence {
                        kind: *kind,
                        value: None,
                    },
                );
            }
        }
        for ((client, kind), value) in next {
            if previous.get(&(client, kind)) != Some(&value) {
                inner.peer_said(
                    replica,
                    client,
                    be_session::SessionMessage::Presence {
                        kind,
                        value: Some(value),
                    },
                );
            }
        }
    }

    pub fn receive_from_peer(
        &self,
        block: Option<Uuid>,
        from: ClientId,
        channel: Uuid,
        payload: Vec<u8>,
    ) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        if let Some(replica) = inner.blocks.get(&key).and_then(|held| held.replica) {
            inner.peer_said(
                replica,
                from,
                be_session::SessionMessage::Channel { channel, payload },
            );
        }
    }

    pub fn defer(&self, defer: bool) {
        let deferred = {
            let mut inner = self.0.borrow_mut();
            match defer {
                true => {
                    inner.deferred.get_or_insert_with(Vec::new);
                    Vec::new()
                }
                false => inner.deferred.take().unwrap_or_default(),
            }
        };
        for (block, editor, message) in deferred {
            self.0.borrow_mut().editor_sent(block, editor, message);
        }
    }

    pub fn seeded(&self) -> Vec<SeededContent> {
        self.0.borrow().seeded.clone()
    }

    fn seed(&self, block: Uuid, content_type: Uuid, bytes: Vec<u8>, replace: bool) {
        let mut inner = self.0.borrow_mut();
        inner.seeded.push(SeededContent {
            block,
            content_type,
            bytes: bytes.clone(),
            replace,
        });
        if replace || !inner.blocks.contains_key(&block) {
            inner.put(block, content_type, bytes);
        }
    }

    pub(crate) fn receive(&self, message: &EditorMessage) {
        match message {
            EditorMessage::Replica {
                instance,
                block_id,
                frame,
            } => {
                if let Some(message) = FromEditor::from_frame(frame) {
                    self.0.borrow_mut().editor_sent(
                        Uuid::from_bytes(*block_id),
                        instance.0,
                        message,
                    );
                }
            }
            EditorMessage::BlockCommand {
                block_id,
                command: command @ (BlockCommand::Undo | BlockCommand::Redo),
                ..
            } => {
                let block = Uuid::from_bytes(*block_id);
                let change = match command {
                    BlockCommand::Redo => Change::Redo,
                    _ => Change::Undo,
                };
                self.0
                    .borrow_mut()
                    .reverting(block, |inner| inner.change(block, change));
            }
            EditorMessage::WatchContent { instance, blocks } => {
                let watched: BTreeSet<Uuid> = blocks
                    .iter()
                    .map(|watched| Uuid::from_bytes(watched.block_id))
                    .collect();
                let mut inner = self.0.borrow_mut();
                let previous = std::mem::replace(&mut inner.watched_content, watched.clone());
                for block in previous.difference(&watched) {
                    if inner.own != Some(*block) {
                        inner.detach(*block, instance.0);
                    }
                }
                for block in watched.difference(&previous) {
                    inner.attach(*block, instance.0);
                }
            }
            EditorMessage::SeedContent {
                block_id,
                content_type,
                bytes,
                ..
            } => self.seed(
                Uuid::from_bytes(*block_id),
                Uuid::from_bytes(*content_type),
                bytes.clone(),
                false,
            ),
            EditorMessage::ReplaceContent {
                block_id,
                content_type,
                bytes,
                ..
            } => self.seed(
                Uuid::from_bytes(*block_id),
                Uuid::from_bytes(*content_type),
                bytes.clone(),
                true,
            ),
            EditorMessage::CreateBlock {
                block_id,
                content_type,
                parent,
                defined,
                content,
                ..
            } => {
                let id = Uuid::from_bytes(*block_id);
                let block_type = Uuid::from_bytes(*content_type);
                let info = BlockInfo::decode(block_plugin_api::BlockInfo {
                    block_id: *block_id,
                    block_type: *content_type,
                    author: Some(Uuid::nil().into_bytes()),
                    parent: Some(*parent),
                    references: Vec::new(),
                    access: block_plugin_api::AccessLevel::Manage,
                    metadata: block_plugin_api::BlockMetadata {
                        defined: defined.clone(),
                        ..block_plugin_api::BlockMetadata::default()
                    },
                    locked: false,
                    deleted: false,
                    sets_access: false,
                });
                {
                    let mut inner = self.0.borrow_mut();
                    inner.graph.insert(id, info);
                    inner.created.push(id);
                }
                match content {
                    Some(bytes) => self.seed(id, block_type, bytes.to_vec(), false),
                    None => {
                        let mut inner = self.0.borrow_mut();
                        if let Reply::Blank(bytes) = inner.request(Request::Blank {
                            content_type: block_type,
                        }) {
                            inner.put(id, block_type, bytes);
                        }
                    }
                }
            }
            EditorMessage::SetParent {
                block_id, parent, ..
            } => {
                if let Some(info) = self
                    .0
                    .borrow_mut()
                    .graph
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    info.parent = Some(BlockParent::decode(*parent));
                }
            }
            EditorMessage::Define {
                block_id,
                key,
                value,
                ..
            } => {
                if let Some(info) = self
                    .0
                    .borrow_mut()
                    .graph
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    info.metadata.define(&[(
                        Uuid::from_bytes(*key),
                        value.as_ref().map(|value| value.to_vec()),
                    )]);
                }
            }
            EditorMessage::WatchHeads { blocks, .. } => {
                self.0.borrow_mut().watched_heads =
                    blocks.iter().copied().map(Uuid::from_bytes).collect();
            }
            EditorMessage::WatchBlocks { queries, .. } => {
                self.0.borrow_mut().watched_queries =
                    queries.iter().copied().map(BlockQuery::decode).collect();
            }
            _ => {}
        }
    }

    fn head_bytes(inner: &mut Inner, block: Uuid) -> Option<(Uuid, Vec<u8>)> {
        let held = inner.blocks.get(&block)?;
        let content_type = held.content_type;
        let bytes = match held.replica {
            Some(replica) => match inner.request(Request::Content { replica }) {
                Reply::Content(bytes) => bytes,
                _ => return None,
            },
            None => inner.commits[&held.head?].clone(),
        };
        Some((content_type, bytes))
    }

    pub(crate) fn outgoing(&self, instance: EditorInstanceId) -> Vec<Message> {
        let mut messages = Vec::new();
        {
            let mut inner = self.0.borrow_mut();
            let outbox = std::mem::take(&mut inner.outbox);
            for (block, editor, frame) in outbox {
                if editor != instance.0 {
                    inner.outbox.push((block, editor, frame));
                    continue;
                }
                let Some(message) = ToEditor::decode(&frame) else {
                    continue;
                };
                messages.push(Message::Editor(EditorMessage::Replica {
                    instance,
                    block_id: block.into_bytes(),
                    frame: message.frame(),
                }));
            }
            let watched: Vec<Uuid> = inner.watched_heads.iter().copied().collect();
            for block in watched {
                let revision = inner.blocks.get(&block).map_or(0, |held| held.revision);
                if inner.heads_sent.insert(block, revision) == Some(revision) {
                    continue;
                }
                let head = Self::head_bytes(&mut inner, block).map(|(content_type, bytes)| {
                    let mut commit = [0; 32];
                    commit[..8].copy_from_slice(&revision.to_be_bytes());
                    StoredHead {
                        commit,
                        content_type: content_type.into_bytes(),
                        bytes,
                    }
                });
                messages.push(Message::Editor(EditorMessage::Head {
                    instance,
                    block_id: block.into_bytes(),
                    head: Ok(head),
                }));
            }
        }
        let queries = self.0.borrow().watched_queries.clone();
        for query in queries {
            if self.leaves_to_the_test(query) {
                continue;
            }
            let listed = self.query(query);
            let mut inner = self.0.borrow_mut();
            if inner.answered.get(&query) == Some(&listed) {
                continue;
            }
            inner.answered.insert(query, listed.clone());
            messages.push(Message::Editor(EditorMessage::Blocks {
                instance,
                query: query.encode(),
                blocks: listed.iter().map(BlockInfo::encode).collect(),
            }));
        }
        messages
    }

    pub(crate) fn pending(&self) -> bool {
        let owed = {
            let inner = self.0.borrow();
            !inner.outbox.is_empty()
                || inner.watched_heads.iter().any(|block| {
                    let revision = inner.blocks.get(block).map_or(0, |held| held.revision);
                    inner.heads_sent.get(block) != Some(&revision)
                })
        };
        owed || {
            let queries = self.0.borrow().watched_queries.clone();
            queries.into_iter().any(|query| {
                if self.leaves_to_the_test(query) {
                    return false;
                }
                let listed = self.query(query);
                self.0.borrow().answered.get(&query) != Some(&listed)
            })
        }
    }

    pub(crate) fn know_elsewhere(&self, known: impl Fn(Uuid) -> bool + 'static) {
        self.0.borrow_mut().known_elsewhere = Some(Rc::new(known));
    }

    fn leaves_to_the_test(&self, query: BlockQuery) -> bool {
        let (BlockQuery::Block(block) | BlockQuery::Parents(block)) = query else {
            return false;
        };
        let inner = self.0.borrow();
        if inner.graph.contains_key(&block) {
            return false;
        }
        let known = inner.known_elsewhere.clone();
        drop(inner);
        known.is_some_and(|known| known(block))
    }

    fn held(&self, nodes: &[BlockInfo], node: &BlockInfo) -> bool {
        match node.parent {
            Some(BlockParent::Root) | None => true,
            Some(BlockParent::Detached) => false,
            Some(BlockParent::Block(parent)) => {
                !nodes.iter().any(|above| above.id == parent)
                    || self.references_of(parent).contains(&node.id)
            }
        }
    }

    fn deleted(&self, nodes: &[BlockInfo], block: Uuid) -> bool {
        let mut current = block;
        for _ in 0..=nodes.len() {
            let Some(node) = nodes.iter().find(|node| node.id == current) else {
                return false;
            };
            if !self.held(nodes, node) {
                return true;
            }
            match node.parent {
                Some(BlockParent::Block(parent)) => current = parent,
                Some(BlockParent::Root | BlockParent::Detached) | None => return false,
            }
        }
        false
    }

    fn references_of(&self, id: Uuid) -> Vec<Uuid> {
        let inner = self.0.borrow();
        inner
            .blocks
            .get(&id)
            .and_then(|held| held.references.clone())
            .or_else(|| inner.graph.get(&id).map(|info| info.references.clone()))
            .unwrap_or_default()
    }

    fn query(&self, query: BlockQuery) -> Vec<BlockInfo> {
        let nodes: Vec<BlockInfo> = self.0.borrow().graph.values().cloned().collect();
        let known = |id: &Uuid| nodes.iter().find(|node| node.id == *id).cloned();
        let with_references = |mut info: BlockInfo| {
            info.references = self.references_of(info.id);
            info
        };
        let listed: Vec<BlockInfo> = match query {
            BlockQuery::Roots => nodes
                .iter()
                .filter(|node| node.parent == Some(BlockParent::Root))
                .cloned()
                .collect(),
            BlockQuery::Shared => nodes
                .iter()
                .filter(|node| {
                    node.access.can_view()
                        && node
                            .parent
                            .and_then(BlockParent::block)
                            .is_some_and(|parent| {
                                !known(&parent).is_some_and(|above| above.access.can_view())
                            })
                })
                .cloned()
                .collect(),
            BlockQuery::Detached => nodes
                .iter()
                .filter(|node| node.parent.is_some() && !self.held(&nodes, node))
                .cloned()
                .collect(),
            BlockQuery::Children(parent) => nodes
                .iter()
                .filter(|node| node.parent == Some(BlockParent::Block(parent)))
                .cloned()
                .collect(),
            BlockQuery::References(block) => {
                self.references_of(block).iter().filter_map(known).collect()
            }
            BlockQuery::Backrefs(block) => nodes
                .iter()
                .filter(|node| self.references_of(node.id).contains(&block))
                .cloned()
                .collect(),
            BlockQuery::Parents(block) => {
                let mut chain = Vec::new();
                let mut current = known(&block).and_then(|node| node.parent?.block());
                while let Some(parent) = current {
                    let Some(node) = known(&parent) else {
                        break;
                    };
                    if chain.iter().any(|seen: &BlockInfo| seen.id == parent) {
                        break;
                    }
                    current = node.parent.and_then(BlockParent::block);
                    chain.push(node);
                }
                chain.reverse();
                chain
            }
            BlockQuery::Block(block) => known(&block).into_iter().collect(),
            BlockQuery::OpenSettings(_) => Vec::new(),
        };
        listed
            .into_iter()
            .map(with_references)
            .map(|mut info| {
                info.deleted = info.deleted || self.deleted(&nodes, info.id);
                info
            })
            .collect()
    }
}
