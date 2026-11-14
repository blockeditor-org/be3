use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use be_block_store::{ChunkerConfig, ContentKey, Hash, Keyring, Manifest, MemoryStore, Vault};
use be_sdk_api::{BlockCommand, EditorInstanceId, StoredHead};
use be_sdk_beui::be_block_content::{LiveEdit, Merge, Undo};
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery, SeededContent};
use be_sdk_type_guest::api::{
    Announcement, Change, ClientId, CommitId, EditorId, Endpoint, Opening, PublishOutcome,
    Recipients, Resume, SessionState, SessionTag,
    embed::{
        exports::replicas::{self, Replica},
        host::Call,
    },
    ops::{
        Backlog, Connection, Incoming, OpError, OpId, Outcome, Outlets, Reply, Side, Ticket,
        guest::Executor, wire,
    },
};
use be_sdk_type_guest::editor::{EditorMirror, FromEditor, Gesture, ToEditor};
use be_sdk_type_guest::wire::Wire;
use be_sdk_type_guest::{Module, Registration};
use uuid::Uuid;

const THIS_DEVICE: ClientId = u64::MAX - 1;
const ELSEWHERE: ClientId = u64::MAX;
const TAG: SessionTag = SessionTag([0; 32]);
const STORE: EditorId = u64::MAX;
const SEALING: Uuid = Uuid::from_u128(0x5ea1);

fn every_type() -> Vec<Registration> {
    be_sdk_type_host::registrations()
        .into_iter()
        .chain([Registration::of::<
            be_sdk_beui::be_block_content::EditorViewContent,
        >()])
        .map(Registration::without_undo)
        .collect()
}

struct Block {
    content_type: Uuid,
    head: Option<CommitId>,
    replica: Option<Replica>,
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

/// What has to happen before the server holds an announced edit, as the host tracks it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Announcing {
    /// In this device's journal as this sequence.
    Journal(u64),
    /// In a commit kept in the outbox.
    Kept,
    /// The server holds it.
    Held(u64),
    /// Never to be held.
    Dropped,
}

struct Inner {
    own: Option<Uuid>,
    organization: Uuid,
    module: Module,
    executor: Executor,
    host: Connection,
    clock: Outlets<()>,
    sleeping: Vec<Reply<()>>,
    now: u64,
    replicas: BTreeMap<OpId, Uuid>,
    /// Each type's blank content, as the module's start listed it.
    blanks: BTreeMap<Uuid, Vec<u8>>,
    commits: BTreeMap<CommitId, Vec<u8>>,
    /// The commits written with parts after their first, as a vault holds them: a type that loads
    /// only its first part reads the rest by range.
    manifests: BTreeMap<CommitId, Manifest>,
    vault: Vault<MemoryStore>,
    next_commit: u64,
    blocks: BTreeMap<Uuid, Block>,
    attached: BTreeMap<Uuid, BTreeSet<EditorId>>,
    presence: BTreeMap<Uuid, BTreeMap<(ClientId, Uuid), Vec<u8>>>,
    outbox: Vec<(Uuid, EditorId, Vec<u8>)>,
    replies: BTreeMap<OpId, Outcome>,
    elsewhere: u64,
    reverted: Option<Vec<Vec<u8>>>,
    seeded: Vec<SeededContent>,
    graph: BTreeMap<Uuid, BlockInfo>,
    created: Vec<Uuid>,
    owner_only: BTreeSet<Uuid>,
    watched_content: BTreeSet<Uuid>,
    watched_heads: BTreeSet<Uuid>,
    heads_sent: BTreeMap<Uuid, u64>,
    watched_queries: Vec<BlockQuery>,
    answered: BTreeMap<BlockQuery, Vec<BlockInfo>>,
    left: BTreeSet<BlockQuery>,
    deferred: Option<Vec<(Uuid, EditorId, FromEditor)>>,
    offline: bool,
    kept: BTreeSet<Uuid>,
    /// The commits the server refused, by block: they stay on the device.
    refused: BTreeMap<Uuid, Vec<CommitId>>,
    refusals: u8,
    /// What edits announce until the server holds them, kept beside the journal or a kept
    /// commit: a simpler host than be-block-client's (one device, so every rebase is on its own
    /// commit; every key remembered; guides/type_modules.md, "Announcements").
    announcing: Vec<(Option<Uuid>, Announcing, Announcement)>,
    /// The keys announced, which are not announced again.
    announced: BTreeSet<(Option<Uuid>, String)>,
    notified: Vec<(Option<Uuid>, String, Vec<Uuid>)>,
    /// Part reads wait (held) until released one at a time, to see a view between reads.
    hold_reads: bool,
    held_reads: Vec<(Option<Uuid>, Call)>,
    known_elsewhere: Option<Rc<dyn Fn(Uuid) -> bool>>,
}

impl Default for Inner {
    fn default() -> Self {
        let module = Module::new(every_type());
        let mut inner = Self {
            own: None,
            organization: Uuid::nil(),
            executor: module.executor(),
            module,
            host: Connection::new(Side::Host),
            clock: Outlets::default(),
            sleeping: Vec::new(),
            now: 0,
            replicas: BTreeMap::new(),
            blanks: BTreeMap::new(),
            commits: BTreeMap::new(),
            manifests: BTreeMap::new(),
            vault: {
                let keys = Keyring::new();
                keys.insert(SEALING, ContentKey::from_bytes([7; 32]));
                Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL)
            },
            next_commit: 0,
            blocks: BTreeMap::new(),
            attached: BTreeMap::new(),
            presence: BTreeMap::new(),
            outbox: Vec::new(),
            replies: BTreeMap::new(),
            elsewhere: 0,
            reverted: None,
            seeded: Vec::new(),
            graph: BTreeMap::new(),
            created: Vec::new(),
            owner_only: BTreeSet::new(),
            watched_content: BTreeSet::new(),
            watched_heads: BTreeSet::new(),
            heads_sent: BTreeMap::new(),
            watched_queries: Vec::new(),
            answered: BTreeMap::new(),
            left: BTreeSet::new(),
            deferred: None,
            offline: false,
            kept: BTreeSet::new(),
            refused: BTreeMap::new(),
            refusals: 0,
            announcing: Vec::new(),
            announced: BTreeSet::new(),
            notified: Vec::new(),
            hold_reads: false,
            held_reads: Vec::new(),
            known_elsewhere: None,
        };
        if let Some(types) = inner.ask(|host| replicas::start(host, vec![0; 32])) {
            inner.blanks = types
                .into_iter()
                .map(|info| (info.content_type, info.blank))
                .collect();
        }
        inner
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

    /// A commit of content with `parts` (`Written`): kept in the vault, so `Load` and `ReadPart`
    /// answer from it. Content written whole, with no parts, is kept as its bytes.
    fn write_written(
        &mut self,
        content_type: Uuid,
        bytes: Vec<u8>,
        parts: &[be_sdk_type_guest::api::WrittenPart],
    ) -> CommitId {
        if parts.is_empty() {
            return self.write(bytes);
        }
        let manifest = self
            .vault
            .write(
                SEALING,
                content_type,
                be_sdk_type_guest::api::written_parts(&bytes, parts),
            )
            .expect("a commit's parts are written");
        let whole = self.vault.read(&manifest).expect("a commit reads whole");
        let commit = self.write(whole);
        self.manifests.insert(commit, manifest);
        commit
    }

    fn run(&mut self) {
        let mut handled = 0;
        loop {
            let frames = self.host.take_outbound();
            if frames.is_empty() {
                return;
            }
            handled += 1;
            assert!(handled < 100_000, "the block's replica never settled");
            let mut outbound = Vec::new();
            self.executor
                .step(frames, &mut outbound)
                .expect("the replica's frames decode");
            for frame in outbound {
                self.host
                    .receive(&frame)
                    .expect("the replica's frames decode");
            }
            while let Some(event) = self.host.next_event() {
                self.heard(event);
            }
        }
    }

    fn heard(&mut self, event: Incoming) {
        match event {
            Incoming::Started(started) => {
                let parent = started.parent;
                if let Some(call) = Call::accept(&mut self.host, started) {
                    let block = parent.and_then(|parent| self.replicas.get(&parent).copied());
                    self.act(block, call);
                }
            }
            Incoming::Completed { op, outcome } => match self.replicas.remove(&op) {
                Some(block) => {
                    if let Some(held) = self.blocks.get_mut(&block)
                        && held.replica.map(Replica::op) == Some(op)
                    {
                        held.replica = None;
                    }
                }
                None => {
                    self.replies.insert(op, outcome);
                }
            },
            Incoming::Canceled { op } => self.host.complete(op, Outcome::Err(OpError::Canceled)),
            Incoming::Credited { op } => {
                self.clock.credited(&mut self.host, op);
            }
            Incoming::Yielded { .. } => {}
        }
    }

    fn act(&mut self, block: Option<Uuid>, call: Call) {
        match call {
            Call::Clock { stream } => {
                self.clock = Outlets::default();
                self.clock.insert(stream, (), Backlog::Latest);
                let _ = self.clock.send(&mut self.host, &self.now, |_| true);
            }
            Call::Send {
                reply,
                to: Recipients::One(Endpoint::Editor(editor)),
                bytes,
                ..
            } => {
                reply.send(&mut self.host, &());
                let Some(block) = block else {
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
            Call::Publish {
                reply,
                expected,
                content,
                ..
            } => {
                let Some(block) = block else {
                    return reply.fail(&mut self.host, OpError::Canceled);
                };
                let head = self.blocks.get(&block).and_then(|held| held.head);
                if expected != head {
                    return reply.send(&mut self.host, &Ok(PublishOutcome::Rejected(head)));
                }
                // Unchanged content keeps the head: its parts' bytes whole are the head's.
                let unchanged = head.is_some_and(|head| {
                    let whole = match content.parts.is_empty() {
                        true => Some(content.bytes.clone()),
                        false => self
                            .vault
                            .write(
                                SEALING,
                                Uuid::nil(),
                                be_sdk_type_guest::api::written_parts(
                                    &content.bytes,
                                    &content.parts,
                                ),
                            )
                            .ok()
                            .and_then(|manifest| self.vault.read(&manifest).ok()),
                    };
                    whole.is_some_and(|whole| whole == self.commits[&head])
                });
                let content_type = self
                    .blocks
                    .get(&block)
                    .map_or(Uuid::nil(), |held| held.content_type);
                let head = match (unchanged, head) {
                    (true, Some(head)) => head,
                    _ => self.write_written(content_type, content.bytes, &content.parts),
                };
                if let Some(held) = self.blocks.get_mut(&block) {
                    held.head = Some(head);
                }
                let outcome = match self.offline {
                    true => {
                        self.kept.insert(block);
                        PublishOutcome::Kept(head)
                    }
                    false => PublishOutcome::Published(head),
                };
                reply.send(&mut self.host, &Ok(outcome));
            }
            Call::Stash { reply, content, .. } => {
                let content_type = block
                    .and_then(|block| self.blocks.get(&block))
                    .map_or(Uuid::nil(), |held| held.content_type);
                let commit = self.write_written(content_type, content.bytes, &content.parts);
                reply.send(&mut self.host, &Ok(commit));
            }
            Call::Load {
                reply,
                commit,
                first_part,
            } if self.manifests.contains_key(&commit) => {
                let manifest = &self.manifests[&commit];
                let loaded = match first_part {
                    true => self.vault.load_first(manifest),
                    false => self.vault.load(manifest),
                };
                let result = loaded
                    .map(be_sdk_type_guest::api::Loaded::of)
                    .map_err(|error| error.to_string());
                reply.send(&mut self.host, &result);
            }
            Call::Load { reply, commit, .. } => {
                // Commits without parts are kept whole, so their content loads all of itself.
                let result = self
                    .commits
                    .get(&commit)
                    .map(|bytes| be_sdk_type_guest::api::Loaded {
                        bytes: bytes.clone(),
                        parts: Vec::new(),
                    })
                    .ok_or_else(|| "no such commit".to_owned());
                reply.send(&mut self.host, &result);
            }
            call @ Call::ReadPart { .. } if self.hold_reads => self.held_reads.push((block, call)),
            Call::ReadPart { reply, part, range } => {
                let result = self
                    .vault
                    .read_part(part.into(), range.offset, range.length)
                    .map_err(|error| error.to_string());
                reply.send(&mut self.host, &result);
            }
            Call::PutPart { reply, .. } => {
                reply.send(
                    &mut self.host,
                    &Err("commits here have no parts".to_owned()),
                );
            }
            Call::ResumeSession { reply, local } => {
                let head = block
                    .and_then(|block| self.blocks.get(&block))
                    .and_then(|held| held.head);
                let resume = match (local, head) {
                    (None, None) => Resume::Empty,
                    (Some(local), Some(head)) if local == head => Resume::UpToDate,
                    (_, Some(head)) => Resume::FastForward { to: head },
                    (Some(local), None) => Resume::Publish { from: local },
                };
                reply.send(&mut self.host, &Ok(resume));
            }
            Call::Described { reply, description } => {
                reply.send(&mut self.host, &());
                if let Some(held) = block.and_then(|block| self.blocks.get_mut(&block)) {
                    held.references = Some(description.references);
                }
            }
            Call::SleepUntil { reply, .. } => self.sleeping.push(reply),
            Call::Append {
                reply,
                announcements,
                ..
            } => {
                let held = !self.offline;
                let until = match held {
                    true => Announcing::Held,
                    false => Announcing::Journal,
                };
                for announced in announcements {
                    self.announcing.push((
                        block,
                        until(announced.sequence),
                        announced.announcement,
                    ));
                }
                self.send_announcements(block);
                reply.send(&mut self.host, &held);
            }
            Call::Announce {
                reply,
                announcements,
            } => {
                for made in announcements {
                    self.announcing.push((block, Announcing::Held(0), made));
                }
                self.send_announcements(block);
                reply.send(&mut self.host, &());
            }
            Call::RebaseJournal { reply, base, .. } => {
                // This device seals every commit here, so what the base holds goes with it.
                let kept = block.is_some_and(|block| self.kept.contains(&block));
                for (at, until, _) in &mut self.announcing {
                    if *at == block
                        && let Announcing::Journal(sequence) = *until
                    {
                        *until = match (sequence <= base.sequence, kept) {
                            (false, _) => Announcing::Dropped,
                            (true, true) => Announcing::Kept,
                            (true, false) => Announcing::Held(sequence),
                        };
                    }
                }
                self.announcing
                    .retain(|(_, until, _)| *until != Announcing::Dropped);
                self.send_announcements(block);
                reply.send(&mut self.host, &());
            }
            Call::Send { reply, .. }
            | Call::ClaimLease { reply, .. }
            | Call::ReportClean { reply, .. }
            | Call::ReportStatus { reply, .. }
            | Call::InStepAt { reply, .. }
            | Call::Conflict { reply, .. }
            | Call::Error { reply, .. } => reply.send(&mut self.host, &()),
        }
    }

    /// Sends what the server holds the edits of in `block`, as the host does: one notification,
    /// the newest's `several` for several, each key once.
    fn send_announcements(&mut self, block: Option<Uuid>) {
        let mut held = Vec::new();
        self.announcing.retain(|(at, until, made)| {
            let sending = *at == block && matches!(until, Announcing::Held(_));
            if sending {
                held.push(made.clone());
            }
            !sending
        });
        held.retain(|made| self.announced.insert((block, made.key.clone())));
        let text = match held.as_slice() {
            [] => return,
            [one] => one.text.clone(),
            several => several[several.len() - 1]
                .several
                .replace("{count}", &several.len().to_string()),
        };
        let mut mentions = Vec::new();
        for mention in held.iter().flat_map(|made| &made.mentions) {
            if !mentions.contains(mention) {
                mentions.push(*mention);
            }
        }
        self.notified.push((block, text, mentions));
    }

    fn ask<T, E: wire::Decode<T>>(
        &mut self,
        call: impl FnOnce(&mut Connection) -> Ticket<T, E>,
    ) -> Option<T> {
        let ticket = call(&mut self.host);
        self.run();
        let outcome = self.replies.remove(&ticket.op())?;
        ticket.decode(outcome).ok()
    }

    fn on_replica(&mut self, replica: Replica, call: impl FnOnce(Replica, &mut Connection)) {
        call(replica, &mut self.host);
        self.run();
    }

    fn open(&mut self, block: Uuid) {
        let organization = self.organization;
        let Some(held) = self.blocks.get_mut(&block) else {
            return;
        };
        if held.replica.is_some() || !self.module.knows(held.content_type) {
            return;
        }
        let (content_type, head) = (held.content_type, held.head);
        let unapplied = std::mem::take(&mut held.unapplied);
        let commits_kept = match self.kept.contains(&block) {
            true => be_sdk_type_guest::api::KeptCommits::Outbox,
            false => be_sdk_type_guest::api::KeptCommits::None,
        };
        let refused_commits = self.refused.get(&block).cloned().unwrap_or_default();
        held.followed.clear();
        let replica = Replica::new(
            &mut self.host,
            Opening {
                block,
                content_type,
                organization,
                account: self.own.unwrap_or_default(),
                client: THIS_DEVICE,
                tag: TAG,
                head,
                session: session(),
                journal: None,
                lost_edits: false,
                journal_kept: false,
                commits_kept,
                refused_commits,
                // This store keeps no head of the server's apart from its own: a reopened
                // replica follows what it carried, never more.
                server_head: head,
                sealed_head: None,
            },
        );
        if let Some(held) = self.blocks.get_mut(&block) {
            held.replica = Some(replica);
        }
        self.replicas.insert(replica.op(), block);
        self.on_replica(replica, |replica, host| {
            replica.attach(host, STORE, false);
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
                be_block_session::SessionMessage::Presence {
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
            self.on_replica(replica, |replica, host| {
                replica.attach(host, editor, true);
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
        self.on_replica(replica, |replica, host| replica.resource_drop(host));
        self.replicas.remove(&replica.op());
        if let Some(held) = self.blocks.get_mut(&block) {
            held.replica = None;
        }
    }

    fn peer_said(
        &mut self,
        replica: Replica,
        client: ClientId,
        message: be_block_session::SessionMessage,
    ) {
        let bytes = Wire::Session(message).encode();
        let now = self.now;
        self.on_replica(replica, |replica, host| {
            replica.message(
                host,
                Endpoint::Peer(client),
                bytes,
                now,
                Some(be_sdk_type_guest::api::Delivery {
                    kind: be_sdk_type_guest::api::RelayKind::Edit,
                    received: now as i64,
                }),
            );
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
        let id = be_block_session::OpId {
            client: ELSEWHERE,
            counter: self.elsewhere,
        };
        self.peer_said(
            replica,
            ELSEWHERE,
            be_block_session::SessionMessage::Submit {
                id,
                base: u64::MAX,
                payload: be_block_session::Payload::Inline(operation),
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
            self.ask(|host| replica.change(host, change));
        }
    }

    fn put(&mut self, block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
        self.put_written(block, content_type, bytes, &[]);
    }

    fn put_written(
        &mut self,
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
        parts: &[be_sdk_type_guest::api::WrittenPart],
    ) {
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
        let head = self.write_written(content_type, bytes, parts);
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
            self.on_replica(replica, |replica, host| {
                replica.attach(host, editor, true);
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
            self.on_replica(replica, |replica, host| {
                replica.detach(host, editor);
            });
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
        if self
            .blocks
            .get(&block)
            .is_some_and(|held| held.replica.is_none())
        {
            self.open(block);
        }
        let Some(replica) = self.blocks.get(&block).and_then(|held| held.replica) else {
            return;
        };
        let send = move |inner: &mut Self| {
            let now = inner.now;
            inner.on_replica(replica, |replica, host| {
                replica.message(host, Endpoint::Editor(editor), frame, now, None);
            });
        };
        match message {
            FromEditor::Gesture(Gesture::Cancel) => self.reverting(block, send),
            _ => send(self),
        }
    }
}

#[derive(Default)]
pub(crate) struct Outgoing {
    pub(crate) heads: Vec<(Uuid, Result<Option<StoredHead>, String>)>,
    pub(crate) listings: Vec<(BlockQuery, Vec<be_sdk_api::BlockInfo>)>,
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

    pub(crate) fn forget_answers(&self) {
        let mut inner = self.0.borrow_mut();
        inner.answered.clear();
        inner.heads_sent.clear();
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

    /// Whether the editor made `block` readable by its account alone.
    pub fn owner_only(&self, block: Uuid) -> bool {
        self.0.borrow().owner_only.contains(&block)
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
        let inner = self.0.borrow_mut();
        for registration in registrations {
            inner.module.register(registration.without_undo());
        }
    }

    fn know<C: LiveEdit + Merge + Clone + Default>(&self) {
        let inner = self.0.borrow_mut();
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
            let inner = &mut *inner;
            let _ = inner.clock.send(&mut inner.host, &now, |_| true);
            inner.run();
        }
    }

    pub fn hold<C: LiveEdit + Merge + Clone + Default>(&self, block: Option<Uuid>, content: C) {
        let key = self.key(block);
        self.know::<C>();
        self.0
            .borrow_mut()
            .put(key, C::CONTENT_TYPE, content.encode());
    }

    /// Like `hold`, but keeps the content's parts after its first (`BlockContent::parts`) in the
    /// store, which a type that loads only its first part reads by range. Held before the block
    /// is open.
    pub fn hold_parts<C: LiveEdit + Merge + Clone + Default>(
        &self,
        block: Option<Uuid>,
        content: C,
    ) {
        let key = self.key(block);
        self.know::<C>();
        let (bytes, parts) = be_sdk_type_guest::api::written_shape(&content.parts())
            .unwrap_or_else(|| (content.encode(), Vec::new()));
        self.0
            .borrow_mut()
            .put_written(key, C::CONTENT_TYPE, bytes, &parts);
    }

    /// Holds part reads back until `release_read` lets them through one at a time, so a test can
    /// look at what an editor shows while only some of the parts it asked for were read.
    pub fn hold_reads(&self, hold: bool) {
        self.0.borrow_mut().hold_reads = hold;
    }

    /// Lets the oldest held part read through; false when none is held.
    pub fn release_read(&self) -> bool {
        let mut inner = self.0.borrow_mut();
        if inner.held_reads.is_empty() {
            return false;
        }
        let (block, call) = inner.held_reads.remove(0);
        let held = std::mem::replace(&mut inner.hold_reads, false);
        inner.act(block, call);
        inner.hold_reads = held;
        inner.run();
        true
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

    /// What the replica asked the host to notify the block's other members of, since last taken:
    /// the block, the text and who it mentions.
    pub fn take_notified(&self) -> Vec<(Uuid, String, Vec<Uuid>)> {
        let mut inner = self.0.borrow_mut();
        let own = inner.own.unwrap_or_default();
        std::mem::take(&mut inner.notified)
            .into_iter()
            .map(|(block, text, mentions)| (block.unwrap_or(own), text, mentions))
            .collect()
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
                    be_block_session::SessionMessage::Presence {
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
                    be_block_session::SessionMessage::Presence {
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
                be_block_session::SessionMessage::Channel { channel, payload },
            );
        }
    }

    /// Takes this device off the server or puts it back, as for an editor's sending state
    /// (`ToEditor::Lacking`): offline, journal appends are answered as kept on the device alone
    /// and a seal as kept in the outbox (`publish-outcome.kept`); back online, the replicas
    /// holding kept commits are told they are `drained`.
    pub fn set_online(&self, online: bool) {
        self.0.borrow_mut().offline = !online;
        if online {
            self.drain(false);
        }
    }

    /// Back online, the server refuses the commits kept offline, as a drain refused for lost
    /// access does: the replicas holding them are told they will not reach it.
    pub fn refuse_kept(&self) {
        self.0.borrow_mut().offline = false;
        self.drain(true);
    }

    fn drain(&self, refused: bool) {
        let mut inner = self.0.borrow_mut();
        let kept: Vec<Uuid> = std::mem::take(&mut inner.kept).into_iter().collect();
        for block in kept {
            let mut commits = Vec::new();
            if refused {
                inner.refusals += 1;
                let commit = CommitId::from_hash(Hash::from_bytes([inner.refusals; 32]));
                inner.refused.entry(block).or_default().push(commit);
                commits.push(commit);
            }
            for (at, until, _) in &mut inner.announcing {
                if *at == Some(block) && *until == Announcing::Kept {
                    *until = match refused {
                        true => Announcing::Dropped,
                        false => Announcing::Held(0),
                    };
                }
            }
            inner
                .announcing
                .retain(|(_, until, _)| *until != Announcing::Dropped);
            inner.send_announcements(Some(block));
            if let Some(replica) = inner.blocks.get(&block).and_then(|held| held.replica) {
                replica.drained(&mut inner.host, refused, commits);
            }
        }
        inner.run();
    }

    /// Closes the block's replica, as the host does once no editor holds it; the next editor
    /// that asks for the block opens it again.
    pub fn close_replica(&self, block: Option<Uuid>) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        inner.close(key);
        inner.run();
    }

    /// Holds the editor's operations back until called with false, to test what the editor
    /// shows before the host has taken an edit.
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

    pub(crate) fn editor_sent(&self, block: Uuid, instance: EditorInstanceId, message: FromEditor) {
        self.0.borrow_mut().editor_sent(block, instance.0, message);
    }

    pub(crate) fn command(&self, block: Uuid, command: &BlockCommand) {
        let change = match command {
            BlockCommand::Undo => Change::Undo,
            BlockCommand::Redo => Change::Redo,
            _ => return,
        };
        self.0
            .borrow_mut()
            .reverting(block, |inner| inner.change(block, change));
    }

    pub(crate) fn watch_content(&self, instance: EditorInstanceId, block: Uuid, watched: bool) {
        let mut inner = self.0.borrow_mut();
        if inner.own == Some(block) {
            return;
        }
        match watched {
            true => {
                if inner.watched_content.insert(block) {
                    inner.attach(block, instance.0);
                }
            }
            false => {
                if inner.watched_content.remove(&block) {
                    inner.detach(block, instance.0);
                }
            }
        }
    }

    pub(crate) fn seed_content(
        &self,
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
        replace: bool,
    ) {
        self.seed(block, content_type, bytes, replace);
    }

    pub(crate) fn create_block(
        &self,
        block_id: [u8; 16],
        content_type: [u8; 16],
        parent: be_sdk_api::BlockLocation,
        defined: be_sdk_api::MetadataEntries,
        content: Option<Vec<u8>>,
        owner_only: bool,
        author: Uuid,
    ) {
        let id = Uuid::from_bytes(block_id);
        let block_type = Uuid::from_bytes(content_type);
        let info = BlockInfo::decode(be_sdk_api::BlockInfo {
            block_id,
            block_type: content_type,
            author: Some(author.into_bytes()),
            parent: Some(parent),
            references: Vec::new(),
            access: be_sdk_api::AccessLevel::Manage,
            metadata: be_sdk_api::BlockMetadata {
                defined,
                ..be_sdk_api::BlockMetadata::default()
            },
            locked: false,
            deleted: false,
            sets_access: false,
            size: 0,
            tree_size: 0,
            out_of_reach: false,
        });
        {
            let mut inner = self.0.borrow_mut();
            inner.graph.insert(id, info);
            inner.created.push(id);
            if owner_only {
                inner.owner_only.insert(id);
            }
        }
        match content {
            Some(bytes) => self.seed(id, block_type, bytes, false),
            None => {
                let mut inner = self.0.borrow_mut();
                if let Some(bytes) = inner.blanks.get(&block_type).cloned() {
                    inner.put(id, block_type, bytes);
                }
            }
        }
    }

    pub(crate) fn set_parent(&self, block: Uuid, parent: be_sdk_api::BlockLocation) {
        if let Some(info) = self.0.borrow_mut().graph.get_mut(&block) {
            info.parent = Some(BlockParent::decode(parent));
        }
    }

    pub(crate) fn define(&self, block: Uuid, key: Uuid, value: Option<Vec<u8>>) {
        if let Some(info) = self.0.borrow_mut().graph.get_mut(&block) {
            info.metadata.define(&[(key, value)]);
        }
    }

    pub(crate) fn watch_heads(&self, blocks: BTreeSet<Uuid>) {
        self.0.borrow_mut().watched_heads = blocks;
    }

    pub(crate) fn watch_queries(&self, queries: Vec<BlockQuery>) {
        self.0.borrow_mut().watched_queries = queries;
    }

    fn head_bytes(inner: &mut Inner, block: Uuid) -> Option<(Uuid, Vec<u8>)> {
        let held = inner.blocks.get(&block)?;
        let content_type = held.content_type;
        let bytes = match held.replica {
            Some(replica) => inner.ask(|host| replica.content(host))??,
            None => inner.commits[&held.head?].clone(),
        };
        Some((content_type, bytes))
    }

    pub(crate) fn outgoing(
        &self,
        instance: EditorInstanceId,
        mut relay: impl FnMut(Uuid, &Vec<u8>) -> bool,
    ) -> Outgoing {
        let mut outgoing = Outgoing::default();
        {
            let mut inner = self.0.borrow_mut();
            let outbox = std::mem::take(&mut inner.outbox);
            let mut blocked = BTreeSet::new();
            for (block, editor, frame) in outbox {
                if editor != instance.0 || blocked.contains(&block) {
                    inner.outbox.push((block, editor, frame));
                    continue;
                }
                let Some(message) = ToEditor::decode(&frame) else {
                    continue;
                };
                if !relay(block, &message.frame()) {
                    blocked.insert(block);
                    inner.outbox.push((block, editor, frame));
                }
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
                outgoing.heads.push((block, Ok(head)));
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
            outgoing
                .listings
                .push((query, listed.iter().map(BlockInfo::encode).collect()));
        }
        outgoing
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

    pub fn leave_listing(&self, query: BlockQuery) {
        self.0.borrow_mut().left.insert(query);
    }

    fn leaves_to_the_test(&self, query: BlockQuery) -> bool {
        if self.0.borrow().left.contains(&query) {
            return true;
        }
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
            Some(BlockParent::Root | BlockParent::Unfiled) | None => true,
            Some(BlockParent::Detached) => false,
            Some(BlockParent::Block(parent)) => {
                !nodes.iter().any(|above| above.id == parent)
                    || self.links(parent).contains(&node.id)
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
                Some(BlockParent::Root | BlockParent::Detached | BlockParent::Unfiled) | None => {
                    return false;
                }
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

    fn links(&self, id: Uuid) -> Vec<Uuid> {
        let mut links = self.references_of(id);
        if let Some(info) = self.0.borrow().graph.get(&id) {
            links.extend(info.metadata.references());
        }
        links
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
            BlockQuery::Unfiled => nodes
                .iter()
                .filter(|node| node.parent == Some(BlockParent::Unfiled))
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
                .filter(|node| self.links(node.id).contains(&block))
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
