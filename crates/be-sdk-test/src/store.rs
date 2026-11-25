use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
use be_sdk_type_guest::holds::{Holds, This, Took};
use be_sdk_type_guest::{Module, Registration};
use uuid::Uuid;

/// Bytes no content type decodes.
const UNREADABLE: &[u8] = &[0xff; 7];

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
    /// The editors' tentative gestures, followed as the host follows them: their edits are
    /// written only when the layer commits.
    holds: Holds,
    /// The held edits, by their serial in `holds`.
    held: BTreeMap<u64, Vec<u8>>,
    /// The block's session's order, as the server would number it.
    order: be_block_session::Sequencer,
    /// The block's signed log, as the server would number and stamp it: one device, so every
    /// entry is this account's and verified.
    log: Vec<be_sdk_type_guest::api::LogItem>,
}

impl Block {
    fn write(&mut self, operation: Vec<u8>) {
        self.written.push(operation);
        self.taken += 1;
        self.revision += 1;
    }

    /// Writes the held edits `took` counts, in the order they arrived, and lets go of the rest
    /// it released.
    fn release(&mut self, took: &Took) {
        for serial in &took.counted {
            if let Some(operation) = self.held.remove(serial) {
                self.write(operation);
            }
        }
        for serial in &took.released {
            self.held.remove(serial);
        }
    }

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
            holds: Holds::default(),
            held: BTreeMap::new(),
            order: be_block_session::Sequencer::starting(0),
            log: Vec::new(),
        }
    }
}

struct Inner {
    own: Option<Uuid>,
    organization: Uuid,
    module: Module,
    executor: Executor,
    host: Connection,
    clock: Outlets<()>,
    sleeping: Vec<Reply<()>>,
    /// Whether the server holds appends to signed logs back (`hold_appends`), as one not
    /// reached yet, and those it holds: each block, entry id, bytes and what goes along.
    hold_appends: bool,
    held_appends: Vec<(Option<Uuid>, Uuid, Vec<u8>, be_sdk_type_guest::api::Along)>,
    /// Why the server refuses appends, while it does (`refuse_appends`).
    refuse_appends: Option<String>,
    /// The appends the server refused that the device keeps until let go: each block, entry
    /// id, bytes and why.
    not_appended: Vec<(Option<Uuid>, Uuid, Vec<u8>, String)>,
    /// How many appends were made, which numbers their ids.
    appends: u64,
    /// The entries logged since the replicas were last handed them, by replica.
    logged: Vec<(Replica, Vec<be_sdk_type_guest::api::LogItem>)>,
    /// Who signed what each account appends, where a test says (`set_log_signer`).
    signers: HashMap<Uuid, be_sdk_beui::be_block_content::Signer>,
    /// Whether the server holds submissions back (`hold_submissions`), and those it holds.
    hold_submissions: bool,
    held_submissions: Vec<(Option<Uuid>, Call)>,
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
    /// How many operations each attached editor sent its block's replicas: what a replica
    /// opened again under it holds of them (the replicas' `attach`).
    forwarded: BTreeMap<(Uuid, EditorId), u64>,
    /// The gestures each editor has open on each block and how each began, which detaching it
    /// ends as the host does when an instance closes: an undo group is committed, a tentative
    /// gesture cancelled.
    gestures: BTreeMap<(Uuid, EditorId), BTreeMap<u64, Gesture>>,
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
    /// The blocks with commits kept in the outbox, with the last one kept.
    kept: BTreeMap<Uuid, CommitId>,
    /// The commits the server refused, by block: they stay on the device.
    refused: BTreeMap<Uuid, Vec<CommitId>>,
    refusals: u8,
    /// What log entries the server took announce, until they are sent: a simpler host than
    /// be-block-client's (every key remembered; guides/type_modules.md, "Announcements").
    announcing: Vec<(Option<Uuid>, Announcement)>,
    /// The keys announced, which are not announced again.
    announced: BTreeSet<(Option<Uuid>, String)>,
    notified: Vec<(Option<Uuid>, String, Vec<Uuid>)>,
    /// Part reads wait (held) until released one at a time, to see a view between reads.
    hold_reads: bool,
    held_reads: Vec<(Option<Uuid>, Call)>,
    known_elsewhere: Option<Rc<dyn Fn(Uuid) -> bool>>,
    /// The account the editor's host is signed in to, whose unread counts it is told
    /// (`BlockInfo::unread`).
    account: Uuid,
}

impl Default for Inner {
    fn default() -> Self {
        let module = Module::new(every_type());
        let mut inner = Self {
            own: None,
            account: Uuid::nil(),
            organization: Uuid::nil(),
            executor: module.executor(),
            module,
            host: Connection::new(Side::Host),
            clock: Outlets::default(),
            sleeping: Vec::new(),
            hold_appends: false,
            held_appends: Vec::new(),
            refuse_appends: None,
            not_appended: Vec::new(),
            appends: 0,
            logged: Vec::new(),
            signers: HashMap::new(),
            hold_submissions: false,
            held_submissions: Vec::new(),
            now: 0,
            replicas: BTreeMap::new(),
            blanks: BTreeMap::new(),
            commits: BTreeMap::new(),
            manifests: BTreeMap::new(),
            vault: {
                let keys = Keyring::new();
                keys.insert(SEALING, ContentKey::from_bytes([7; 32]), Uuid::nil());
                Vault::new(MemoryStore::new(), keys).with_chunker(ChunkerConfig::SMALL)
            },
            next_commit: 0,
            blocks: BTreeMap::new(),
            attached: BTreeMap::new(),
            forwarded: BTreeMap::new(),
            gestures: BTreeMap::new(),
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
            kept: BTreeMap::new(),
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

fn session(tag: SessionTag) -> SessionState {
    SessionState {
        index: 1,
        after: 0,
        participants: vec![be_sdk_type_guest::api::Participant {
            client: THIS_DEVICE,
            tag: Hash::from_bytes(tag.0),
            may_edit: true,
            claimed: None,
        }],
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
                        self.kept.insert(block, head);
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
                    .map_err(|error| {
                        be_sdk_type_guest::api::LoadError::Unreadable(error.to_string())
                    });
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
                    .ok_or_else(|| {
                        be_sdk_type_guest::api::LoadError::Unreadable("no such commit".to_owned())
                    });
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
            Call::Append {
                reply,
                bytes,
                along,
            } => {
                self.appends += 1;
                let id = uuid::Uuid::from_u64_pair(0xa99e_d000, self.appends);
                let held = self.offline || self.hold_appends;
                // Refused as the server takes it: the device keeps it until let go.
                if let (false, Some(why)) = (held, self.refuse_appends.clone()) {
                    self.not_appended.push((block, id, bytes, why.clone()));
                    let not = be_sdk_type_guest::api::host::NotAppended { id: Some(id), why };
                    return reply.send(
                        &mut self.host,
                        &Err::<be_sdk_type_guest::api::host::Appended, _>(not),
                    );
                }
                let place = match held {
                    true => {
                        self.held_appends.push((block, id, bytes, along));
                        None
                    }
                    false => self
                        .log_entry(block, id, self.account, bytes, along, None)
                        .map(|(seq, arrival)| be_sdk_type_guest::api::host::LogPlace {
                            seq,
                            arrival,
                        }),
                };
                reply.send(
                    &mut self.host,
                    &Ok::<_, be_sdk_type_guest::api::host::NotAppended>(
                        be_sdk_type_guest::api::host::Appended { id, place },
                    ),
                );
                self.deliver_logged();
            }
            Call::ReadLog { reply, from, limit } => {
                let log = block
                    .or(self.own)
                    .and_then(|block| self.blocks.get(&block))
                    .map(|held| held.log.clone())
                    .unwrap_or_default();
                let items = log
                    .iter()
                    .skip(from as usize)
                    .take(limit as usize)
                    .cloned()
                    .collect();
                let kept = self
                    .held_appends
                    .iter()
                    .filter(|(at, ..)| *at == block)
                    .map(|(_, id, bytes, _)| be_sdk_type_guest::api::KeptEntry {
                        id: *id,
                        payload: bytes.clone(),
                    })
                    .collect();
                let page = be_sdk_type_guest::api::LogPage {
                    size: log.len() as u64,
                    items,
                    kept,
                    refused: self.refused_entries(block),
                };
                reply.send(&mut self.host, &Ok(page));
            }
            Call::LetGoRefused { reply, id } => {
                self.not_appended
                    .retain(|(at, refused, ..)| *at != block || *refused != id);
                reply.send(&mut self.host, &());
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
            call @ Call::Submit { .. } if !self.offline && self.hold_submissions => {
                self.held_submissions.push((block, call));
            }
            Call::Submit { reply, submission } => {
                let held = !self.offline;
                let entry = self.number(block, submission, held);
                let answer = match held {
                    true => be_sdk_type_guest::api::host::Submitted::Held,
                    false => be_sdk_type_guest::api::host::Submitted::Kept,
                };
                reply.send(&mut self.host, &answer);
                if let Some(replica) = block
                    .and_then(|block| self.blocks.get(&block))
                    .and_then(|held| held.replica)
                {
                    self.on_replica(replica, |replica, host| {
                        replica.sequenced(host, vec![entry]);
                    });
                }
            }
            Call::Send { reply, .. }
            | Call::RebaseJournal { reply, .. }
            | Call::Claim { reply }
            | Call::ReportStatus { reply, .. }
            | Call::InStepAt { reply, .. }
            | Call::Conflict { reply, .. }
            | Call::Error { reply, .. } => reply.send(&mut self.host, &()),
        }
    }

    /// Sends what the server holds the entries of in `block`, as the host does: one
    /// notification, the newest's `several` for several, each key once.
    fn send_announcements(&mut self, block: Option<Uuid>) {
        let mut held = Vec::new();
        self.announcing.retain(|(at, made)| {
            let sending = *at == block;
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

    /// Puts an entry `author` wrote in `block`'s log as the server would, numbered and stamped,
    /// with what goes along with it: what it announces is sent, and what it takes back loses
    /// its payload. Its replica hears it, with what it took back, on `deliver_logged`. Its place
    /// and arrival, or none for a block not held.
    fn log_entry(
        &mut self,
        block: Option<Uuid>,
        id: Uuid,
        author: Uuid,
        bytes: Vec<u8>,
        along: be_sdk_type_guest::api::Along,
        arrival: Option<i64>,
    ) -> Option<(u64, i64)> {
        let key = block.or(self.own)?;
        let held = self.blocks.get_mut(&key)?;
        let seq = held.log.len() as u64;
        // The server's clock, as the test's wall clock reads it now.
        let arrival = arrival
            .unwrap_or((beui_core::timer::TEST_WALL_CLOCK.as_millis() as u64 + self.now) as i64);
        let item = be_sdk_type_guest::api::LogItem {
            seq,
            id,
            author,
            arrival,
            payload: Some(bytes),
            signer: self.signer_of(author),
            imported: None,
        };
        let held = self.blocks.get_mut(&key)?;
        held.log.push(item.clone());
        // What it takes back loses its payload, as the server's redaction does.
        let redacted = along
            .redacts
            .and_then(|seq| held.log.get_mut(seq as usize))
            .filter(|taken| taken.author == author)
            .map(|taken| {
                taken.payload = None;
                taken.signer = be_sdk_beui::be_block_content::Signer::Redacted;
                taken.clone()
            });
        if let Some(replica) = held.replica {
            self.logged
                .push((replica, [item].into_iter().chain(redacted).collect()));
        }
        if let Some(announcement) = along.announcement {
            self.announcing.push((block, announcement));
            if self.held_appends.is_empty() {
                self.send_announcements(block);
            }
        }
        Some((seq, arrival))
    }

    /// Who signed what `author` appended, as the host's verdict: a test's `set_log_signer`, or a
    /// contact's key.
    fn signer_of(&self, author: Uuid) -> be_sdk_beui::be_block_content::Signer {
        match self.signers.get(&author) {
            Some(signer) => *signer,
            None => be_sdk_beui::be_block_content::Signer::Verified,
        }
    }

    /// The entries of `block` the server refused that the device keeps, as a read hands them.
    fn refused_entries(&self, block: Option<Uuid>) -> Vec<be_sdk_type_guest::api::RefusedEntry> {
        self.not_appended
            .iter()
            .filter(|(at, ..)| *at == block)
            .map(|(_, id, bytes, why)| be_sdk_type_guest::api::RefusedEntry {
                id: *id,
                payload: bytes.clone(),
                why: why.clone(),
            })
            .collect()
    }

    /// The server takes the appends it held, in order, or refuses them all while it refuses
    /// appends (`refuse_appends`): the device keeps those, and the replica of each block open
    /// hears every refused entry it keeps, as a drain tells.
    /// What they announce goes as one per block, as a drain sends it.
    fn release_appends(&mut self) {
        if let Some(why) = self.refuse_appends.clone() {
            let held = std::mem::take(&mut self.held_appends);
            let mut blocks = Vec::new();
            for (block, id, bytes, _) in held {
                self.not_appended.push((block, id, bytes, why.clone()));
                if !blocks.contains(&block) {
                    blocks.push(block);
                }
            }
            for block in blocks {
                let entries = self.refused_entries(block);
                if let Some(replica) = block
                    .or(self.own)
                    .and_then(|key| self.blocks.get(&key))
                    .and_then(|held| held.replica)
                {
                    self.on_replica(replica, |replica, host| {
                        replica.refused(host, entries);
                    });
                }
            }
            return;
        }
        let held = std::mem::take(&mut self.held_appends);
        let mut blocks = Vec::new();
        for (index, (block, id, bytes, along)) in held.iter().cloned().enumerate() {
            let author = self.account;
            // Each but the last is taken as one more the drain holds back.
            self.held_appends = held[index + 1..].to_vec();
            self.log_entry(block, id, author, bytes, along, None);
            if !blocks.contains(&block) {
                blocks.push(block);
            }
        }
        self.held_appends.clear();
        for block in blocks {
            self.send_announcements(block);
        }
        self.deliver_logged();
    }

    /// Hands each replica the entries its log took since this was last called.
    fn deliver_logged(&mut self) {
        for (replica, items) in std::mem::take(&mut self.logged) {
            self.on_replica(replica, |replica, host| {
                replica.logged(host, items);
            });
        }
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
        let commits_kept = match self.kept.contains_key(&block) {
            true => be_sdk_type_guest::api::KeptCommits::Outbox,
            false => be_sdk_type_guest::api::KeptCommits::None,
        };
        let refused_commits = self.refused.get(&block).cloned().unwrap_or_default();
        held.followed.clear();
        // The replica opens on the head with no journal: the order starts over from it.
        held.order = be_block_session::Sequencer::starting(0);
        let replica = Replica::new(
            &mut self.host,
            Opening {
                block,
                content_type,
                organization,
                account: self.own.unwrap_or_default(),
                client: THIS_DEVICE,
                tag: TAG,
                origin: self.next_commit + 1,
                taken_before: 0,
                doubtful_through: 0,
                kept_restart: 0,
                kept_through: 0,
                kept_submitted: 0,
                kept_submissions: Vec::new(),
                head,
                session: session(TAG),
                journal: None,
                journal_kept: false,
                commits_kept,
                refused_commits,
                // This store keeps no head of the server's apart from its own: a reopened
                // replica follows what it carried, never more.
                server_head: head,
                sealed_head: None,
                epoch: 0,
            },
        );
        if let Some(held) = self.blocks.get_mut(&block) {
            held.replica = Some(replica);
        }
        self.replicas.insert(replica.op(), block);
        self.on_replica(replica, |replica, host| {
            replica.attach(host, STORE, false, 0);
            replica.resent(host, STORE);
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
            let held = self.forwarded_by(block, editor);
            self.on_replica(replica, |replica, host| {
                replica.attach(host, editor, true, held);
                replica.resent(host, editor);
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
        let bytes = be_sdk_type_guest::wire::encode(&message);
        let now = self.now;
        self.on_replica(replica, |replica, host| {
            replica.message(host, Endpoint::Peer(client), bytes, now);
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
        let submission = be_sdk_type_guest::api::Submission {
            origin: ELSEWHERE,
            counter: self.elsewhere,
            base: u64::MAX,
            barrier: false,
            payload: be_sdk_type_guest::wire::Submitted {
                through: 0,
                entry: be_sdk_type_guest::wire::Entry::Operation(
                    be_block_session::Payload::Inline(operation),
                ),
            }
            .encode(),
        };
        let entry = self.number(Some(block), submission, true);
        self.on_replica(replica, |replica, host| {
            replica.sequenced(host, vec![entry]);
        });
    }

    /// Numbers `submission` in `block`'s order, as the server would (or this device offline,
    /// with no time).
    fn number(
        &mut self,
        block: Option<Uuid>,
        submission: be_sdk_type_guest::api::Submission,
        online: bool,
    ) -> be_sdk_type_guest::api::JournalEntry {
        let now = self.now as i64;
        let order = match block.and_then(|block| self.blocks.get_mut(&block)) {
            Some(held) => &mut held.order,
            None => {
                return be_sdk_type_guest::api::JournalEntry {
                    sequence: 0,
                    origin: submission.origin,
                    counter: submission.counter,
                    base: submission.base,
                    floor: 0,
                    received: None,
                    payload: submission.payload,
                };
            }
        };
        let op = order.number(
            be_block_session::Submission {
                id: be_block_session::OpId {
                    origin: submission.origin,
                    counter: submission.counter,
                },
                base: submission.base,
                barrier: submission.barrier,
                payload: submission.payload,
            },
            online.then_some(now),
        );
        be_sdk_type_guest::api::JournalEntry {
            sequence: op.sequence,
            origin: op.id.origin,
            counter: op.id.counter,
            base: op.base,
            floor: op.floor,
            received: op.received,
            payload: op.payload,
        }
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
        if matches!(change, Change::Undo)
            && let Some(held) = self.blocks.get_mut(&block)
        {
            let took = held.holds.undo();
            held.release(&took);
        }
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
            let held = self.forwarded_by(block, editor);
            self.on_replica(replica, |replica, host| {
                replica.attach(host, editor, true, held);
                replica.resent(host, editor);
            });
        }
        true
    }

    fn forwarded_by(&self, block: Uuid, editor: EditorId) -> u64 {
        self.forwarded
            .get(&(block, editor))
            .copied()
            .unwrap_or_default()
    }

    fn detach(&mut self, block: Uuid, editor: EditorId) {
        for (id, begun) in self.gestures.remove(&(block, editor)).unwrap_or_default() {
            let gesture = match begun {
                Gesture::BeginTentative => Gesture::Cancel,
                _ => Gesture::Commit,
            };
            self.editor_sent(block, editor, FromEditor::Gesture { id, gesture });
        }
        self.forwarded.remove(&(block, editor));
        if let Some(held) = self.blocks.get_mut(&block) {
            let took = held.holds.detach(editor);
            held.release(&took);
        }
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
        if let FromEditor::Gesture { id, gesture } = &message {
            let open = self.gestures.entry((block, editor)).or_default();
            match gesture {
                Gesture::Begin | Gesture::BeginTentative => {
                    open.insert(*id, *gesture);
                }
                Gesture::Commit | Gesture::Cancel => {
                    open.remove(id);
                }
            }
        }
        if attached && message == FromEditor::Resend {
            return;
        }
        if let (FromEditor::Operate { .. }, Some(deferred)) = (&message, &mut self.deferred) {
            deferred.push((block, editor, message));
            return;
        }
        if let FromEditor::Operate { .. } | FromEditor::Gesture { .. } = &message {
            let Some(held) = self.blocks.get_mut(&block) else {
                return;
            };
            let operation = match &message {
                FromEditor::Operate { operation, .. } => Some(operation.clone()),
                _ => None,
            };
            if operation.is_some() && held.replica.is_none() {
                return;
            }
            let took = held.holds.message(editor, Some(&message));
            match (operation, took.this) {
                (Some(operation), This::Held(serial)) => {
                    held.held.insert(serial, operation);
                }
                (Some(operation), _) => held.write(operation),
                (None, _) => {}
            }
            held.release(&took);
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
        if let FromEditor::Operate { .. } = &message {
            *self.forwarded.entry((block, editor)).or_default() += 1;
        }
        let send = move |inner: &mut Self| {
            let now = inner.now;
            inner.on_replica(replica, |replica, host| {
                replica.message(host, Endpoint::Editor(editor), frame, now);
            });
        };
        match message {
            FromEditor::Gesture {
                gesture: Gesture::Cancel,
                ..
            } => self.reverting(block, send),
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

    /// `bytes` of `content_type` as the plain file its registered type writes it as, named
    /// after `name` unless the file names itself, as be-host's `file-of-content` answers.
    pub fn file_of(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
        name: &str,
    ) -> Result<be_sdk_beui::SavedFile, be_sdk_api::Failure> {
        use be_sdk_type_guest::api::guest::exports::replicas::Guest as _;
        let inner = self.0.borrow();
        let mut asked = inner.module.to_file(content_type, bytes);
        let std::task::Poll::Ready(file) = asked
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        else {
            panic!("a type module makes a file at once");
        };
        let file = file
            .map_err(|failure| be_sdk_api::Failure::Failed(format!("{failure:?}")))?
            .ok_or_else(|| {
                be_sdk_api::Failure::Failed("This block has no plain-file form.".to_owned())
            })?;
        Ok(be_sdk_beui::SavedFile {
            name: file.name.unwrap_or_else(|| {
                be_sdk_beui::be_block_content::plain::export_name(name, &file.extension)
            }),
            mime_type: file.media_type,
            data: file.bytes,
        })
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

    /// Holds content of type `C` that does not decode in this build, so the block's replica
    /// cannot open it, as the host's replica cannot open a block stored in a form it cannot
    /// read: its editors read it as failed (`Unready::Failed`).
    pub fn hold_unreadable<C: LiveEdit + Merge + Clone + Default>(&self, block: Option<Uuid>) {
        let key = self.key(block);
        self.know::<C>();
        self.0
            .borrow_mut()
            .put(key, C::CONTENT_TYPE, UNREADABLE.to_vec());
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

    /// Holds appends to signed logs back, as a server not reached yet: each is answered as kept
    /// on the device; released, the server takes those it held, in order.
    pub fn hold_appends(&self, hold: bool) {
        let mut inner = self.0.borrow_mut();
        inner.hold_appends = hold;
        if !hold && !inner.offline {
            inner.release_appends();
        }
    }

    /// Has the server refuse appends to signed logs, saying `why`, until called with none.
    pub fn refuse_appends(&self, why: Option<&str>) {
        self.0.borrow_mut().refuse_appends = why.map(str::to_owned);
    }

    /// Puts an entry `author` wrote in `block`'s signed log, as another device's append the
    /// server took at `arrival` (by its clock now, for none): its place.
    pub fn log_entry(
        &self,
        block: Option<Uuid>,
        author: Uuid,
        bytes: Vec<u8>,
        arrival: Option<i64>,
    ) -> u64 {
        let mut inner = self.0.borrow_mut();
        inner.appends += 1;
        let id = uuid::Uuid::from_u64_pair(0xa99e_d000, inner.appends);
        let along = be_sdk_type_guest::api::Along {
            announcement: None,
            redacts: None,
        };
        let place = inner.log_entry(block, id, author, bytes, along, arrival);
        inner.deliver_logged();
        place.map_or(0, |(seq, _)| seq)
    }

    /// Puts an entry of history an import brought in `block`'s signed log, as `importer`
    /// appended it: written by `author` at `original`'s place in the block `from`, verified
    /// there. Its place here.
    pub fn log_imported(
        &self,
        block: Option<Uuid>,
        author: Uuid,
        bytes: Vec<u8>,
        importer: Uuid,
        (from, original): (Uuid, u64),
    ) -> u64 {
        let seq = self.log_entry(block, importer, bytes, None);
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        if let Some(item) = inner
            .blocks
            .get_mut(&key)
            .and_then(|held| held.log.get_mut(seq as usize))
        {
            item.author = author;
            item.imported = Some(be_sdk_type_guest::api::LogImport {
                block: from,
                seq: original,
                importer,
            });
        }
        seq
    }

    /// `block`'s signed log as the server holds it.
    pub fn log(&self, block: Option<Uuid>) -> Vec<be_sdk_type_guest::api::LogItem> {
        let key = self.key(block);
        self.0
            .borrow()
            .blocks
            .get(&key)
            .map(|held| held.log.clone())
            .unwrap_or_default()
    }

    /// Redacts the entry at `seq` of `block`'s signed log as a manager or the server would,
    /// with no entry of its author's naming it: its replica hears it at once.
    pub fn redact_entry(&self, block: Option<Uuid>, seq: u64) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let Some(held) = inner.blocks.get_mut(&key) else {
            return;
        };
        let Some(taken) = held.log.get_mut(seq as usize) else {
            return;
        };
        taken.payload = None;
        taken.signer = be_sdk_beui::be_block_content::Signer::Redacted;
        let taken = taken.clone();
        if let Some(replica) = held.replica {
            inner.logged.push((replica, vec![taken]));
        }
        inner.deliver_logged();
    }

    /// The appends to `block`'s signed log the server holds back, each id and its bytes.
    pub fn held_appends(&self, block: Option<Uuid>) -> Vec<(Uuid, Vec<u8>)> {
        let key = self.key(block);
        let inner = self.0.borrow();
        inner
            .held_appends
            .iter()
            .filter(|(at, ..)| at.or(inner.own) == Some(key))
            .map(|(_, id, bytes, _)| (*id, bytes.clone()))
            .collect()
    }

    /// The ids of the appends to `block`'s signed log the server refused that the device keeps
    /// until let go.
    pub fn refused_appends(&self, block: Option<Uuid>) -> Vec<Uuid> {
        let inner = self.0.borrow();
        inner
            .not_appended
            .iter()
            .filter(|(at, ..)| at.or(inner.own) == Some(self.key(block)))
            .map(|(_, id, ..)| *id)
            .collect()
    }

    /// Says who signed what `author` appends to a signed log, as the host would tell.
    pub fn set_log_signer(&self, author: Uuid, signer: be_sdk_beui::be_block_content::Signer) {
        self.0.borrow_mut().signers.insert(author, signer);
    }

    /// Holds submissions back, as a server that has not answered yet; released, it takes those
    /// it held in order, so a test can look at an edit sent and not yet held by the server.
    pub fn hold_submissions(&self, hold: bool) {
        let mut inner = self.0.borrow_mut();
        inner.hold_submissions = hold;
        if !hold {
            for (block, call) in std::mem::take(&mut inner.held_submissions) {
                inner.act(block, call);
            }
            inner.run();
        }
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
            let mut inner = self.0.borrow_mut();
            if !inner.hold_appends {
                inner.release_appends();
            }
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
        let kept: Vec<(Uuid, CommitId)> = std::mem::take(&mut inner.kept).into_iter().collect();
        for (block, head) in kept {
            // A drain names the head it published; a refusal, the commits it refused.
            let mut commits = match refused {
                true => Vec::new(),
                false => vec![head],
            };
            if refused {
                inner.refusals += 1;
                let commit = CommitId::from_hash(Hash::from_bytes([inner.refusals; 32]));
                inner.refused.entry(block).or_default().push(commit);
                commits.push(commit);
            }
            if let Some(replica) = inner.blocks.get(&block).and_then(|held| held.replica) {
                replica.drained(&mut inner.host, refused, commits);
            }
        }
        inner.run();
    }

    /// Detaches every editor attached to the block's replica, as the host does when their
    /// instances close: the gestures they left open are cancelled.
    pub fn detach_editors(&self, block: Option<Uuid>) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let editors: Vec<EditorId> = inner
            .attached
            .get(&key)
            .map(|held| held.iter().copied().collect())
            .unwrap_or_default();
        for editor in editors {
            inner.detach(key, editor);
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

    /// Starts the editor's replica of `block` afresh, with a snapshot, as the host does when a
    /// later replica of the block ends (`BlockStore::replica_resumed`): what was queued for the
    /// replica that ended goes, and the store's replica answers a resend on its behalf.
    pub(crate) fn resume_content(&self, instance: EditorInstanceId, block: Uuid) {
        let mut inner = self.0.borrow_mut();
        inner
            .outbox
            .retain(|(of, editor, _)| (*of, *editor) != (block, instance.0));
        inner.editor_sent(block, instance.0, FromEditor::Resend);
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
            unread: 0,
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

    /// The account the editor's host is signed in to, as be-host's stack knows its own.
    pub(crate) fn sign_in(&self, account: Uuid) {
        self.0.borrow_mut().account = account;
    }

    /// How many things in `block` the account has not read, as be-host counts them for a type
    /// that counts (guides/blocks/host.md, "Unread counts"): from its head and every
    /// block of the type's reads kind the store holds, this account's layers of it.
    fn unread_of(&self, block: Uuid) -> u32 {
        let mut inner = self.0.borrow_mut();
        let Some(content_type) = inner.blocks.get(&block).map(|held| held.content_type) else {
            return 0;
        };
        let Some(kind) = inner.module.reads_of(content_type) else {
            return 0;
        };
        let layers: Vec<Uuid> = inner
            .blocks
            .iter()
            .filter(|(_, held)| held.content_type == kind)
            .map(|(id, _)| *id)
            .collect();
        let reads: Vec<Vec<u8>> = layers
            .into_iter()
            .filter_map(|layer| Self::head_bytes(&mut inner, layer).map(|(_, bytes)| bytes))
            .collect();
        // Its head, as be-host counts it, never its replica: asking an open replica for its
        // content would step it in the middle of what the test holds back (a part read).
        let Some(bytes) = inner
            .blocks
            .get(&block)
            .and_then(|held| held.head)
            .and_then(|head| inner.commits.get(&head).cloned())
        else {
            return 0;
        };
        let log = inner
            .blocks
            .get(&block)
            .map_or(&[][..], |held| &held.log[..]);
        let latest = &log[log
            .len()
            .saturating_sub(be_sdk_beui::be_block_content::UNREAD_ENTRIES as usize)..];
        inner
            .module
            .unread_of_bytes(content_type, &bytes, &reads, latest, block, inner.account)
            .unwrap_or_default()
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
            info.unread = self.unread_of(info.id);
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
