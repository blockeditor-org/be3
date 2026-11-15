use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_protocol::{
    BlockSummary, ErrorCode, EscrowKey, Journal, JournalBase, JournalEntry, KeyRow, KeySet,
    Listing, Organization, OrganizationKind, OrganizationRole, WrappedBy, WrappedKey,
};
use be_block_store::{Hash, StoreError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

mod announcements;
mod sqlite;

use announcements::Waiting;
pub(crate) use announcements::{Announcing, Made, Until};

type Backend = sqlite::SqliteRefs;

const VERSION: u8 = 1;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Scope {
    pub server: String,
    pub account: Uuid,
    pub organization: Uuid,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum Table {
    Session,
    Head,
    Remote,
    Listing,
    Key,
    Wrap,
    Contacts,
    Outbox,
    Unsent,
    Workspaces,
    JournalBase,
    JournalEntry,
    Signers,
    Announcement,
    Summary,
    Held,
    LetGo,
    ContactNames,
    /// The objects a local account's block holds, one row each (`Refs::grow`).
    Objects,
    OpenEdits,
    SetAside,
    Dropped,
}

impl Table {
    pub const ALL: [Self; 22] = [
        Self::Session,
        Self::Head,
        Self::Remote,
        Self::Listing,
        Self::Key,
        Self::Wrap,
        Self::Contacts,
        Self::Outbox,
        Self::Unsent,
        Self::Workspaces,
        Self::JournalBase,
        Self::JournalEntry,
        Self::Signers,
        Self::Announcement,
        Self::Summary,
        Self::Held,
        Self::LetGo,
        Self::ContactNames,
        Self::Objects,
        Self::OpenEdits,
        Self::SetAside,
        Self::Dropped,
    ];

    pub fn code(self) -> u8 {
        match self {
            Self::Session => 1,
            Self::Head => 2,
            Self::Remote => 3,
            Self::Listing => 4,
            Self::Key => 5,
            Self::Wrap => 6,
            Self::Contacts => 7,
            Self::Outbox => 8,
            Self::Unsent => 9,
            Self::Workspaces => 10,
            Self::JournalBase => 11,
            Self::JournalEntry => 12,
            Self::Signers => 13,
            Self::Announcement => 14,
            Self::Summary => 15,
            Self::Held => 16,
            Self::LetGo => 17,
            Self::ContactNames => 18,
            Self::Objects => 19,
            Self::OpenEdits => 20,
            Self::SetAside => 21,
            Self::Dropped => 22,
        }
    }

    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|table| table.code() == code)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub table: Table,
    pub key: Vec<u8>,
    pub value: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct RefsDb {
    backend: Arc<Backend>,
}

impl Default for RefsDb {
    fn default() -> Self {
        Self::memory()
    }
}

impl RefsDb {
    pub fn memory() -> Self {
        Self {
            backend: Arc::new(Backend::in_memory().expect("an in-memory database always opens")),
        }
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        Ok(Self {
            backend: Arc::new(Backend::open(path)?),
        })
    }

    fn account_scope(server: &str, account: Uuid) -> Scope {
        Scope {
            server: server.to_owned(),
            account,
            organization: Uuid::nil(),
        }
    }

    pub fn workspaces(&self, server: &str, account: Uuid) -> Result<Vec<Organization>, StoreError> {
        let rows = self.backend.load(&Self::account_scope(server, account))?;
        rows.into_iter()
            .find(|row| row.table == Table::Workspaces)
            .and_then(|row| row.value)
            .map_or(Ok(Vec::new()), |value| decode(&value))
    }

    pub fn set_workspaces(
        &self,
        server: &str,
        account: Uuid,
        workspaces: &[Organization],
    ) -> Result<(), StoreError> {
        self.backend.write(
            &Self::account_scope(server, account),
            &[Row {
                table: Table::Workspaces,
                key: Vec::new(),
                value: Some(encode(&workspaces.to_vec())),
            }],
        )
    }

    pub fn knows(
        &self,
        server: &str,
        account: Uuid,
        organization: Uuid,
    ) -> Result<bool, StoreError> {
        let scope = Scope {
            server: server.to_owned(),
            account,
            organization,
        };
        Ok(self
            .backend
            .load(&scope)?
            .iter()
            .any(|row| row.table == Table::Session))
    }

    pub(crate) fn scope(&self, scope: Scope) -> Result<Refs, StoreError> {
        let rows = self.backend.load(&scope)?;
        let state = State::load(rows);
        Ok(Refs {
            backend: Arc::clone(&self.backend),
            scope,
            state: Mutex::new(state),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Session {
    pub role: OrganizationRole,
    pub kind: OrganizationKind,
    pub escrows: Vec<EscrowKey>,
    pub pinned: crate::contacts::Pinned,
    pub server: Option<be_block_protocol::ServerAccount>,
    /// The server's clock less this device's, as last measured (`Peer::server_now`), for
    /// stamping commits offline after a restart.
    pub server_offset: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pending {
    pub block: Uuid,
    pub commit: CommitId,
    pub expected: Option<CommitId>,
    pub parents: Vec<CommitId>,
    /// Every object the commit holds besides itself, and below every block it references:
    /// sending it, `Peer::send_pending` turns both into changes from the expected head's.
    pub chunks: Vec<Hash>,
    pub time: i64,
    pub pinned: bool,
    pub references: Vec<Uuid>,
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Creation {
    pub block: Uuid,
    pub content_type: Uuid,
    pub parent: BlockParent,
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
    pub references: Vec<Uuid>,
    pub key: Uuid,
    pub keys: KeySet,
    pub owner_only: bool,
    /// The root of the block's access log, made with it (`be_block_keys::Entry::root`).
    #[serde(with = "serde_bytes")]
    pub root: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Moving {
    pub block: Uuid,
    pub parent: BlockParent,
    /// Where it was, so sending it can carry its databases' prefixes along (`follow_prefixes`).
    pub from: Option<BlockParent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Defining {
    pub block: Uuid,
    #[serde(with = "serde_bytes")]
    pub metadata: Vec<u8>,
    pub references: Vec<Uuid>,
}

/// Objects held for a block (`Peer::hold`) made offline, uploaded and held once the drain sends
/// it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Holding {
    pub block: Uuid,
    pub objects: Vec<Hash>,
    /// Held for good (`Hold::Lasting`), or until a commit that uses them publishes.
    pub lasting: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Op {
    Create(Creation),
    Publish(Pending),
    Move(Moving),
    Define(Defining),
    Hold(Holding),
}

impl Op {
    pub fn block(&self) -> Uuid {
        match self {
            Self::Create(creation) => creation.block,
            Self::Publish(pending) => pending.block,
            Self::Move(moving) => moving.block,
            Self::Define(defining) => defining.block,
            Self::Hold(holding) => holding.block,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Unsent {
    pub head: Option<CommitId>,
    pub since: i64,
    pub ops: Vec<Op>,
    /// Why it was set aside: the server refused it, with the code it gave (a block it would
    /// not make, over a quota or too large), or else this account lost access to it.
    pub refused: Option<ErrorCode>,
}

/// Entries set aside because the block's access was refused. Only `Peer::discard_unsent`
/// deletes anything here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsentBlock {
    pub block: Uuid,
    pub head: Option<CommitId>,
    pub commits: Vec<CommitId>,
    pub since: i64,
    pub offerable: bool,
    /// The code the server refused it with, when it was refused rather than this account
    /// losing access (`Unsent::refused`): what lost access does to it does not apply.
    pub refused: Option<ErrorCode>,
    /// How many local edits a replica of it made that this device keeps and that never became
    /// a commit (`OpenEdits`): discarding drops them too.
    pub kept_edits: usize,
    /// Its name as this device has it.
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PendingCounts {
    pub operations: usize,
    pub commits: usize,
    pub blocks: usize,
    pub unsent: usize,
}

/// What a catch-up of a root's search index listed of a block (`Peer::changed_page`): its root,
/// its type and the head it was listed at.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListedBlock {
    pub root: Uuid,
    pub content_type: Uuid,
    pub head: Option<CommitId>,
}

/// A signing key this device knows an account by (plans/authorship.md): verified when a
/// contact's card or key change named it, otherwise pinned the first time the server did.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KnownSigner {
    pub key: [u8; 32],
    pub verified: bool,
}

#[derive(Default)]
struct State {
    session: Option<Session>,
    heads: HashMap<Uuid, CommitId>,
    remotes: HashMap<Uuid, CommitId>,
    /// The newest head of each block whose content this device has read whole.
    held: HashMap<Uuid, CommitId>,
    /// Each block's newest summary: what every listing that holds the block shows of it.
    summaries: HashMap<Uuid, BlockSummary>,
    listings: HashMap<Listing, Vec<Uuid>>,
    /// The server's revision when it answered each `Children` and `Deleted` listing: a summary
    /// newer than that is of a block made or moved since (`placed_in`).
    listed_at: HashMap<Listing, u64>,
    /// The blocks each parent holds, as their summaries place them.
    placed: HashMap<BlockParent, HashSet<Uuid>>,
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<(Uuid, WrappedBy), (u64, WrappedKey)>,
    contacts: Vec<(Uuid, [u8; 32])>,
    /// Every contact's name as the server last listed it, so a push opened with no connection
    /// names its sender (`Peer::contact_name`).
    contact_names: Vec<(Uuid, String)>,
    signers: HashMap<Uuid, Vec<KnownSigner>>,
    outbox: BTreeMap<u64, Op>,
    unsent: BTreeMap<Uuid, Unsent>,
    journals: HashMap<Uuid, Journaled>,
    /// What edits made here announce, by block and key, until sent and for a while after.
    announcements: HashMap<Uuid, BTreeMap<String, Waiting>>,
    /// Blocks the search indexer let go after their commit could not be read, with what a
    /// catch-up listed of them: read again when the next catch-up starts, across restarts.
    let_go: HashMap<Uuid, Option<ListedBlock>>,
    /// The distinct objects each of a local account's blocks holds, which its `size` counts
    /// (`grow`).
    objects: HashMap<Uuid, HashSet<Hash>>,
    open_edits: HashMap<Uuid, OpenEdits>,
    set_aside: HashMap<Uuid, Vec<SetAside>>,
    /// Blocks whose local edits were discarded with their unsent edits (`drop_local_edits`):
    /// none are kept for them until this account may edit them again or they are forgotten.
    dropped: HashSet<Uuid>,
    next: u64,
}

/// A block's journal on this device, sealed as on the server: what this device's replica
/// sequenced as its owner since the base, kept until a commit holds it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct JournalHead {
    tag: Hash,
    base: JournalBase,
    /// Where the order stands: what this device heard the server number, or numbered itself
    /// offline.
    order: be_block_session::Sequencer,
}

/// A block's replica origin on this device and its local edits no journal holds yet, as the
/// host gave them its replica: kept so every replica of the block on this device submits as one
/// origin, whose count goes on, and one opened after the host stopped is given them again, as
/// one opened again in the same run is (guides/blocks/host.md, "Opening again").
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct OpenEdits {
    /// The replica's origin, which its submissions name.
    pub origin: u64,
    /// How many local edits came before the first of `edits` (`opening.taken-before`).
    pub given_up: u64,
    /// The last of them submitted to the server, which may hold it.
    pub submitted_through: u64,
    /// How many times the session had started over for the replica that made them
    /// (`status.restarts`).
    pub restarts: u64,
    /// Each edit, a `KeptEdit` sealed with the block's key, in the order given.
    pub edits: Vec<Vec<u8>>,
}

/// Kept edits of a block that could not be read when it opened (`Refs::set_aside`): never
/// sent, and kept until they read and are told so, then only as a tombstone.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SetAside {
    /// Names it among the block's, as the newest are kept.
    pub id: Uuid,
    /// How many edits were set aside.
    pub count: usize,
    /// The edits, until they read and were told so.
    pub kept: Option<OpenEdits>,
}

impl SetAside {
    /// How many a block keeps, the newest.
    pub const KEPT: usize = 3;
}

/// A local edit the host gave a replica: an editor's frame, or a change the host made.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeptEdit {
    Frame { editor: u64, bytes: Vec<u8> },
    Change(KeptChange),
}

/// A change the host asked of a replica (`be_sdk_type_api::Change`), as kept.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeptChange {
    AddChild(Uuid),
    DeleteChild(Uuid),
    ReplaceChild { old: Uuid, new: Uuid },
    ReorderChild { block: Uuid, after: Option<Uuid> },
    FromFile { name: String, bytes: Vec<u8> },
    Replace(Vec<u8>),
    Undo,
    Redo,
}

#[derive(Clone, Debug)]
struct Journaled {
    head: JournalHead,
    entries: BTreeMap<u64, JournalEntry>,
}

fn journal_entry_key(block: Uuid, sequence: u64) -> Vec<u8> {
    let mut key = block.as_bytes().to_vec();
    key.extend(sequence.to_be_bytes());
    key
}

impl State {
    fn load(rows: Vec<Row>) -> Self {
        let mut state = Self::default();
        for row in rows {
            let Some(value) = &row.value else { continue };
            let _ = state.add(&row, value);
        }
        for summary in state.summaries.values() {
            if let Some(parent) = summary.parent {
                state.placed.entry(parent).or_default().insert(summary.id);
            }
        }
        state
    }

    /// Keeps `summary` as its block's, moving it in `placed` from its old parent to its new one.
    fn keep(&mut self, summary: BlockSummary) {
        let id = summary.id;
        let parent = summary.parent;
        if let Some(old) = self
            .summaries
            .insert(id, summary)
            .and_then(|old| old.parent)
            && Some(old) != parent
        {
            self.unplace(old, id);
        }
        if let Some(parent) = parent {
            self.placed.entry(parent).or_default().insert(id);
        }
    }

    fn drop_summary(&mut self, block: Uuid) -> bool {
        let Some(old) = self.summaries.remove(&block) else {
            return false;
        };
        if let Some(parent) = old.parent {
            self.unplace(parent, block);
        }
        true
    }

    /// Takes `block` out of what `parent` holds, and `parent` out of the index once it holds
    /// nothing.
    fn unplace(&mut self, parent: BlockParent, block: Uuid) {
        if let Some(held) = self.placed.get_mut(&parent) {
            held.remove(&block);
            if held.is_empty() {
                self.placed.remove(&parent);
            }
        }
    }

    fn add(&mut self, row: &Row, value: &[u8]) -> Result<(), StoreError> {
        match row.table {
            Table::Session => self.session = Some(decode(value)?),
            Table::Head => {
                self.heads.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Remote => {
                self.remotes.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Held => {
                self.held.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Objects => {
                let (block, object) = row.key.split_at_checked(16).ok_or(StoreError::Corrupt)?;
                let object: [u8; 32] = object.try_into().map_err(|_| StoreError::Corrupt)?;
                self.objects
                    .entry(uuid_key(block)?)
                    .or_default()
                    .insert(Hash::from_bytes(object));
            }
            Table::Listing => {
                let listing = decode_raw(&row.key)?;
                let (at, ids): (u64, Vec<Uuid>) = decode(value)?;
                self.listed_at.insert(listing, at);
                self.listings.insert(listing, ids);
            }
            Table::Summary => {
                self.summaries.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Key => {
                let key: KeyRow = decode(value)?;
                self.keys.insert(key.id, key);
            }
            Table::Wrap => {
                let (sequence, wrap): (u64, WrappedKey) = decode(value)?;
                self.next = self.next.max(sequence + 1);
                self.wraps
                    .insert((wrap.key, wrap.wrapped_by), (sequence, wrap));
            }
            Table::Contacts => self.contacts = decode(value)?,
            Table::ContactNames => self.contact_names = decode(value)?,
            Table::Signers => {
                self.signers.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Outbox => {
                let sequence = sequence_key(&row.key)?;
                self.next = self.next.max(sequence + 1);
                self.outbox.insert(sequence, decode(value)?);
            }
            Table::Unsent => {
                self.unsent.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Workspaces => {}
            Table::OpenEdits => {
                self.open_edits.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::SetAside => {
                self.set_aside.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Dropped => {
                self.dropped.insert(uuid_key(&row.key)?);
            }
            Table::LetGo => {
                self.let_go.insert(uuid_key(&row.key)?, decode(value)?);
            }
            Table::Announcement => {
                let (block, key) = row.key.split_at_checked(16).ok_or(StoreError::Corrupt)?;
                let key = String::from_utf8(key.to_vec()).map_err(|_| StoreError::Corrupt)?;
                let waiting: Waiting = decode(value)?;
                self.next = self.next.max(waiting.order + 1);
                self.announcements
                    .entry(uuid_key(block)?)
                    .or_default()
                    .insert(key, waiting);
            }
            Table::JournalBase => {
                let head: JournalHead = decode(value)?;
                let block = uuid_key(&row.key)?;
                let entries = self
                    .journals
                    .remove(&block)
                    .map(|journal| journal.entries)
                    .unwrap_or_default();
                self.journals.insert(block, Journaled { head, entries });
            }
            Table::JournalEntry => {
                let (block, sequence) = row.key.split_at_checked(16).ok_or(StoreError::Corrupt)?;
                let entry: JournalEntry = decode(value)?;
                let block = uuid_key(block)?;
                let sequence = sequence_key(sequence)?;
                self.journals
                    .entry(block)
                    .or_insert_with(|| Journaled {
                        head: JournalHead {
                            tag: Hash::from_bytes([0; 32]),
                            base: JournalBase {
                                commit: None,
                                sequence: 0,
                                state: Vec::new(),
                            },
                            order: be_block_session::Sequencer::starting(0),
                        },
                        entries: BTreeMap::new(),
                    })
                    .entries
                    .insert(sequence, entry);
            }
        }
        Ok(())
    }

    fn take(&mut self) -> u64 {
        let sequence = self.next;
        self.next += 1;
        sequence
    }
}

pub(crate) struct Refs {
    backend: Arc<Backend>,
    scope: Scope,
    state: Mutex<State>,
}

/// Announced changes of references resolve against the summaries the server last gave
/// (`Connection::hold_with`).
impl crate::connection::Held for Refs {
    fn references(&self, block: Uuid) -> Option<(u64, Vec<Uuid>)> {
        let state = self.state.lock().unwrap();
        state
            .summaries
            .get(&block)
            .map(|summary| (summary.revision, summary.references.clone()))
    }

    fn keep(&self, summary: &BlockSummary) {
        let _ = self.set_listing(Listing::Block(summary.id), std::slice::from_ref(summary));
    }
}

impl Refs {
    fn write(&self, rows: Vec<Row>) -> Result<(), StoreError> {
        if rows.is_empty() {
            return Ok(());
        }
        self.backend.write(&self.scope, &rows)
    }

    pub fn session(&self) -> Option<Session> {
        self.state.lock().unwrap().session.clone()
    }

    pub fn set_session(&self, session: Session) -> Result<(), StoreError> {
        let row = row(Table::Session, Vec::new(), &session);
        {
            let mut state = self.state.lock().unwrap();
            if state.session.as_ref() == Some(&session) {
                return Ok(());
            }
            state.session = Some(session);
        }
        self.write(vec![row])
    }

    pub fn head(&self, block: Uuid) -> Option<CommitId> {
        self.state.lock().unwrap().heads.get(&block).copied()
    }

    pub fn set_head(&self, block: Uuid, head: CommitId) -> Result<(), StoreError> {
        if self.state.lock().unwrap().heads.insert(block, head) == Some(head) {
            return Ok(());
        }
        self.write(vec![row(Table::Head, block.as_bytes().to_vec(), &head)])
    }

    pub fn set_head_to(&self, block: Uuid, head: Option<CommitId>) -> Result<(), StoreError> {
        match head {
            Some(head) => self.set_head(block, head),
            None => {
                if self.state.lock().unwrap().heads.remove(&block).is_none() {
                    return Ok(());
                }
                self.write(vec![Row {
                    table: Table::Head,
                    key: block.as_bytes().to_vec(),
                    value: None,
                }])
            }
        }
    }

    /// Moves the remote head to `head`, unless it is `behind` the one known
    /// (`behind(head, known)`): the server's head only moves on, so an older one was heard of
    /// late.
    pub fn set_remote(
        &self,
        block: Uuid,
        head: CommitId,
        behind: impl Fn(CommitId, CommitId) -> bool,
    ) -> Result<(), StoreError> {
        let (mut state, news) = self.judged(&[(block, head)], behind);
        if !news.contains(&block) {
            return Ok(());
        }
        state.remotes.insert(block, head);
        drop(state);
        self.write(vec![row(Table::Remote, block.as_bytes().to_vec(), &head)])
    }

    /// The state, locked once each of `heads` is judged against the remote head it would
    /// replace, with the blocks whose head is news: neither the one known nor `behind` it.
    /// `behind` walks history, so it is asked with the lock let go, and asked again if the
    /// remote head it was asked about moved meanwhile.
    fn judged(
        &self,
        heads: &[(Uuid, CommitId)],
        behind: impl Fn(CommitId, CommitId) -> bool,
    ) -> (std::sync::MutexGuard<'_, State>, HashSet<Uuid>) {
        let mut judged: HashMap<(Uuid, CommitId), (CommitId, bool)> = HashMap::new();
        loop {
            let state = self.state.lock().unwrap();
            let mut news = HashSet::new();
            let mut unjudged = Vec::new();
            for &(block, head) in heads {
                match state.remotes.get(&block).copied() {
                    None => {
                        news.insert(block);
                    }
                    Some(known) if known == head => {}
                    Some(known) => match judged.get(&(block, head)) {
                        Some(&(asked, behind)) if asked == known => {
                            if !behind {
                                news.insert(block);
                            }
                        }
                        _ => unjudged.push((block, head, known)),
                    },
                }
            }
            if unjudged.is_empty() {
                return (state, news);
            }
            drop(state);
            for (block, head, known) in unjudged {
                judged.insert((block, head), (known, behind(head, known)));
            }
        }
    }

    /// The server holds `head` of `block`: drops `settled` from the outbox and moves the remote
    /// head to it unless it is `behind` (`set_remote`), and the local head too when it
    /// `follows` the local head, all in one step. A reader never sees the outbox emptied while
    /// the heads still name what was there before.
    pub fn settle(
        &self,
        block: Uuid,
        settled: &[u64],
        head: CommitId,
        behind: impl Fn(CommitId, CommitId) -> bool,
        follows: impl FnOnce(Option<CommitId>) -> bool,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let (mut state, news) = self.judged(&[(block, head)], behind);
            for sequence in settled {
                if state.outbox.remove(sequence).is_some() {
                    rows.push(Row {
                        table: Table::Outbox,
                        key: sequence.to_be_bytes().to_vec(),
                        value: None,
                    });
                }
            }
            if news.contains(&block) {
                state.remotes.insert(block, head);
                rows.push(row(Table::Remote, block.as_bytes().to_vec(), &head));
            }
            let local = state.heads.get(&block).copied();
            if follows(local) && state.heads.insert(block, head) != Some(head) {
                rows.push(row(Table::Head, block.as_bytes().to_vec(), &head));
            }
        }
        self.write(rows)
    }

    /// Keeps the heads `summaries` name as the server's, but not one `behind` the remote head
    /// already known (`set_remote`). Where `summaries` name a block more than once, only the
    /// newest head is judged, from the latest summary and, between two of one revision, the one
    /// the other is `behind`: an older one judged against the same known head could otherwise
    /// land after it.
    pub fn observe(
        &self,
        summaries: &[BlockSummary],
        behind: impl Fn(CommitId, CommitId) -> bool,
    ) -> Result<(), StoreError> {
        let mut latest: HashMap<Uuid, (u64, CommitId)> = HashMap::new();
        for summary in summaries {
            let Some(head) = summary.head else { continue };
            let newest = latest.get(&summary.id).is_none_or(|(revision, kept)| {
                *revision < summary.revision
                    || (*revision == summary.revision && behind(*kept, head))
            });
            if newest {
                latest.insert(summary.id, (summary.revision, head));
            }
        }
        let heads: Vec<(Uuid, CommitId)> = latest
            .into_iter()
            .map(|(block, (_, head))| (block, head))
            .collect();
        let mut rows = Vec::new();
        {
            let (mut state, news) = self.judged(&heads, &behind);
            for (block, head) in heads {
                if news.contains(&block) && state.remotes.insert(block, head) != Some(head) {
                    rows.push(row(Table::Remote, block.as_bytes().to_vec(), &head));
                }
            }
        }
        self.write(rows)
    }

    pub fn remote(&self, block: Uuid) -> Option<CommitId> {
        self.state.lock().unwrap().remotes.get(&block).copied()
    }

    pub fn held(&self, block: Uuid) -> Option<CommitId> {
        self.state.lock().unwrap().held.get(&block).copied()
    }

    pub fn hold(&self, block: Uuid, head: CommitId) -> Result<(), StoreError> {
        if self.state.lock().unwrap().held.insert(block, head) == Some(head) {
            return Ok(());
        }
        self.write(vec![row(Table::Held, block.as_bytes().to_vec(), &head)])
    }

    /// `summary` with what the outbox has yet to tell the server of its block: where a waiting
    /// move puts it, and the metadata a waiting definition gives it. Kept as the server said it,
    /// an answer the server gave before those reached it would undo them here.
    pub fn with_outbox(&self, summary: BlockSummary) -> BlockSummary {
        Self::with_outbox_locked(&self.state.lock().unwrap(), summary)
    }

    fn with_outbox_locked(state: &State, mut summary: BlockSummary) -> BlockSummary {
        for op in state.outbox.values().filter(|op| op.block() == summary.id) {
            match op {
                Op::Move(moving) => summary.parent = Some(moving.parent),
                Op::Define(defining) => summary.metadata = defining.metadata.clone(),
                Op::Create(_) | Op::Publish(_) | Op::Hold(_) => {}
            }
        }
        summary
    }

    /// Keeps a summary unless this device already has a newer one of the block. Server answers
    /// go through `set_listing_locked`, which lays the outbox over them; `put` keeps this
    /// device's own writes as given.
    fn put(state: &mut State, summary: &BlockSummary, rows: &mut Vec<Row>) {
        let newer = state
            .summaries
            .get(&summary.id)
            .is_some_and(|known| known.revision > summary.revision);
        if newer || state.summaries.get(&summary.id) == Some(summary) {
            return;
        }
        state.keep(summary.clone());
        rows.push(row(Table::Summary, summary.id.as_bytes().to_vec(), summary));
    }

    fn put_listing(state: &mut State, listing: Listing, ids: Vec<Uuid>, rows: &mut Vec<Row>) {
        if state.listings.get(&listing) == Some(&ids) {
            return;
        }
        let at = state.listed_at.get(&listing).copied().unwrap_or_default();
        rows.push(row(Table::Listing, encode_raw(&listing), &(at, &ids)));
        state.listings.insert(listing, ids);
    }

    pub fn set_listing(&self, listing: Listing, blocks: &[BlockSummary]) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        Self::set_listing_locked(&mut self.state.lock().unwrap(), listing, blocks, &mut rows);
        self.write(rows)
    }

    /// Keeps a listing the server answered, each summary in it with what the outbox has yet to
    /// send laid over it (`with_outbox`): every summary the server gives is kept through here.
    fn set_listing_locked(
        state: &mut State,
        listing: Listing,
        blocks: &[BlockSummary],
        rows: &mut Vec<Row>,
    ) {
        for summary in blocks {
            let summary = Self::with_outbox_locked(state, summary.clone());
            Self::put(state, &summary, rows);
        }
        let ids: Vec<Uuid> = blocks.iter().map(|summary| summary.id).collect();
        Self::put_listing(state, listing, ids, rows);
    }

    /// Keeps a listing as the server answered it at `revision`, which a `Children` or `Deleted`
    /// listing keeps too (`placed_in`).
    pub fn listed(
        &self,
        listing: Listing,
        blocks: &[BlockSummary],
        revision: u64,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            if matches!(listing, Listing::Children(_) | Listing::Deleted)
                && state.listed_at.insert(listing, revision) != Some(revision)
                && let Some(ids) = state.listings.get(&listing).cloned()
            {
                // The listing's row holds the revision, so it is written again even unchanged.
                rows.push(row(Table::Listing, encode_raw(&listing), &(revision, &ids)));
            }
            Self::set_listing_locked(&mut state, listing, blocks, &mut rows);
        }
        self.write(rows)
    }

    pub fn add_to_listing(
        &self,
        listing: Listing,
        summary: &BlockSummary,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            Self::put(&mut state, summary, &mut rows);
            let mut ids = state.listings.get(&listing).cloned().unwrap_or_default();
            if !ids.contains(&summary.id) {
                ids.push(summary.id);
            }
            Self::put_listing(&mut state, listing, ids, &mut rows);
        }
        self.write(rows)
    }

    /// Changes a block's summary on this device, as of the newest revision it knows, and moves it
    /// between the listings of its old and new parent.
    pub fn revise(&self, mut summary: BlockSummary) -> Result<(), StoreError> {
        let block = summary.id;
        let parent = summary.parent;
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            summary.revision = newest(&state).max(summary.revision);
            Self::put(&mut state, &summary, &mut rows);
            let listings: Vec<Listing> = state.listings.keys().copied().collect();
            for listing in listings {
                let mut ids = state.listings[&listing].clone();
                let leaves = match listing {
                    Listing::Children(under) => Some(under) != parent,
                    Listing::Deleted => parent != Some(BlockParent::Detached),
                    _ => false,
                };
                let joins = match listing {
                    Listing::Children(under) => Some(under) == parent,
                    Listing::Deleted => parent == Some(BlockParent::Detached),
                    _ => false,
                };
                if leaves {
                    ids.retain(|held| *held != block);
                } else if joins && !ids.contains(&block) {
                    ids.push(block);
                }
                Self::put_listing(&mut state, listing, ids, &mut rows);
            }
            Self::put_listing(&mut state, Listing::Block(block), vec![block], &mut rows);
        }
        self.write(rows)
    }

    pub fn lose_access(&self, block: Uuid) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            announcements::forget(&mut state, block, &mut rows);
            let Some(mut summary) = state.summaries.get(&block).cloned() else {
                drop(state);
                return self.write(rows);
            };
            summary.access = be_block_graph::Access::None;
            state.keep(summary.clone());
            rows.push(row(Table::Summary, block.as_bytes().to_vec(), &summary));
        }
        self.write(rows)
    }

    pub fn listing(&self, listing: &Listing) -> Option<Vec<BlockSummary>> {
        let state = self.state.lock().unwrap();
        let ids = state.listings.get(listing)?;
        Some(
            ids.iter()
                .filter_map(|id| state.summaries.get(id).cloned())
                .collect(),
        )
    }

    /// A listing as this device last knew it, for answering offline, as its summaries say now:
    /// what a block references is its summary's references (one pushed or revised after the
    /// listing was fetched names them then), children and deleted blocks are placed by their
    /// summaries (`placed_in`), and backrefs are the summaries that reference the block.
    pub fn cached_listing(&self, listing: Listing) -> Option<Vec<BlockSummary>> {
        match listing {
            Listing::References(block) => {
                let Some(referrer) = self.summary(block) else {
                    return self.listing(&listing);
                };
                Some(
                    referrer
                        .references
                        .iter()
                        .filter_map(|reference| self.summary(*reference))
                        .filter(|summary| summary.access.can_know_exists())
                        .collect(),
                )
            }
            Listing::Backrefs(block) => {
                let state = self.state.lock().unwrap();
                state.listings.get(&listing)?;
                Some(
                    state
                        .summaries
                        .values()
                        .filter(|summary| summary.references.contains(&block))
                        .cloned()
                        .collect(),
                )
            }
            Listing::Children(parent) => self.placed_in(listing, parent),
            Listing::Deleted => self.placed_in(listing, BlockParent::Detached),
            _ => self.listing(&listing),
        }
    }

    /// A `Children` or `Deleted` listing this device fetched before, as summaries now place its
    /// blocks: those it listed that are still under `parent`, then those whose summary is newer
    /// than the listing and puts them there (made or moved since, by this device online or
    /// pushed from the server, so in no listing it fetched). A block this device may no longer
    /// know of is left out.
    fn placed_in(&self, listing: Listing, parent: BlockParent) -> Option<Vec<BlockSummary>> {
        let state = self.state.lock().unwrap();
        let listed = state.listings.get(&listing)?;
        let at = state.listed_at.get(&listing).copied().unwrap_or_default();
        let empty = HashSet::new();
        let there = state.placed.get(&parent).unwrap_or(&empty);
        // A block the server refused to make is kept only for its unsent row, not listed.
        let shown = |summary: &&BlockSummary| {
            summary.access.can_know_exists()
                && !state
                    .unsent
                    .get(&summary.id)
                    .is_some_and(|unsent| unsent.refused.is_some())
        };
        let mut placed: Vec<BlockSummary> = listed
            .iter()
            .filter(|id| there.contains(id))
            .filter_map(|id| state.summaries.get(id))
            .filter(shown)
            .cloned()
            .collect();
        let listed: HashSet<&Uuid> = listed.iter().collect();
        let mut since: Vec<BlockSummary> = there
            .iter()
            .filter(|id| !listed.contains(id))
            .filter_map(|id| state.summaries.get(id))
            .filter(|summary| summary.revision > at)
            .filter(shown)
            .cloned()
            .collect();
        since.sort_by_key(|summary| summary.id);
        placed.extend(since);
        Some(placed)
    }

    /// A local account's answer to `listing` (`Peer::local`), from every summary it holds, in
    /// the order of block ids as a server's is: with no other member, it may see every block,
    /// and no settings are hidden from it to open.
    pub fn local_listing(&self, listing: Listing) -> Vec<BlockSummary> {
        let state = self.state.lock().unwrap();
        let placed = |parent: BlockParent| -> Vec<BlockSummary> {
            state
                .summaries
                .values()
                .filter(|summary| summary.parent == Some(parent))
                .cloned()
                .collect()
        };
        let mut found = match listing {
            Listing::Children(parent) => placed(parent),
            Listing::Deleted => placed(BlockParent::Detached),
            Listing::Block(block) => state.summaries.get(&block).cloned().into_iter().collect(),
            Listing::References(block) => state
                .summaries
                .get(&block)
                .map(|summary| {
                    summary
                        .references
                        .iter()
                        .filter_map(|reference| state.summaries.get(reference).cloned())
                        .collect()
                })
                .unwrap_or_default(),
            Listing::Backrefs(block) => state
                .summaries
                .values()
                .filter(|summary| summary.references.contains(&block))
                .cloned()
                .collect(),
            Listing::Shared | Listing::OpenSettings(_) => Vec::new(),
        };
        found.sort_by_key(|summary| summary.id);
        found.dedup_by_key(|summary| summary.id);
        found
    }

    /// Of `objects`, those a local account's `block` does not hold yet (`grow`).
    pub fn unheld_objects(&self, block: Uuid, objects: &[Hash]) -> Vec<Hash> {
        let state = self.state.lock().unwrap();
        let held = state.objects.get(&block);
        let mut unheld: Vec<Hash> = objects
            .iter()
            .filter(|object| held.is_none_or(|held| !held.contains(object)))
            .copied()
            .collect();
        unheld.sort_unstable();
        unheld.dedup();
        unheld
    }

    /// A local account's `block` holds `objects` too, `bytes` of them in all: its `size` and
    /// the `tree_size` of it and of each block above it grow by that, as a server counts them
    /// (`size` the distinct objects the block holds, `tree_size` its own and its children's).
    pub fn grow(&self, block: Uuid, objects: &[Hash], bytes: u64) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let held = state.objects.entry(block).or_default();
            for object in objects {
                if held.insert(*object) {
                    let mut key = block.as_bytes().to_vec();
                    key.extend_from_slice(object.as_bytes());
                    rows.push(Row {
                        table: Table::Objects,
                        key,
                        value: Some(Vec::new()),
                    });
                }
            }
            if bytes > 0 {
                if let Some(mut summary) = state.summaries.get(&block).cloned() {
                    summary.size = summary.size.saturating_add(bytes);
                    state.keep(summary.clone());
                    rows.push(row(Table::Summary, block.as_bytes().to_vec(), &summary));
                }
                Self::add_to_tree(
                    &mut state,
                    Some(BlockParent::Block(block)),
                    bytes as i128,
                    &mut rows,
                );
            }
        }
        self.write(rows)
    }

    /// A local account's `block` moved from `from` to `to`: its `tree_size` leaves the blocks
    /// above where it was and joins those above where it is.
    pub fn carry_tree(
        &self,
        block: Uuid,
        from: Option<BlockParent>,
        to: BlockParent,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let size = state
                .summaries
                .get(&block)
                .map_or(0, |summary| summary.tree_size) as i128;
            if size > 0 && from != Some(to) {
                Self::add_to_tree(&mut state, from, -size, &mut rows);
                Self::add_to_tree(&mut state, Some(to), size, &mut rows);
            }
        }
        self.write(rows)
    }

    /// Adds `bytes` to the `tree_size` of `at`'s block and of every block above it.
    fn add_to_tree(state: &mut State, at: Option<BlockParent>, bytes: i128, rows: &mut Vec<Row>) {
        let mut at = at.and_then(BlockParent::block);
        let mut seen = HashSet::new();
        while let Some(block) = at.filter(|block| seen.insert(*block)) {
            let Some(mut summary) = state.summaries.get(&block).cloned() else {
                break;
            };
            summary.tree_size =
                (summary.tree_size as i128 + bytes).clamp(0, u64::MAX as i128) as u64;
            at = summary.parent.and_then(BlockParent::block);
            state.keep(summary.clone());
            rows.push(row(Table::Summary, block.as_bytes().to_vec(), &summary));
        }
    }

    pub fn newest_revision(&self) -> u64 {
        newest(&self.state.lock().unwrap())
    }

    /// Offline answers, each as of the newest revision this device knows.
    pub fn current(&self, mut blocks: Vec<BlockSummary>) -> Vec<BlockSummary> {
        let newest = self.newest_revision();
        for summary in &mut blocks {
            summary.revision = newest;
        }
        blocks
    }

    pub fn summary(&self, block: Uuid) -> Option<BlockSummary> {
        self.state.lock().unwrap().summaries.get(&block).cloned()
    }

    pub fn summaries(&self) -> Vec<BlockSummary> {
        self.state
            .lock()
            .unwrap()
            .summaries
            .values()
            .cloned()
            .collect()
    }

    pub fn keys(&self) -> KeySet {
        let state = self.state.lock().unwrap();
        let mut wraps: Vec<_> = state.wraps.values().cloned().collect();
        wraps.sort_by_key(|(sequence, _)| *sequence);
        KeySet {
            keys: state.keys.values().copied().collect(),
            wrapped: wraps.into_iter().map(|(_, wrap)| wrap).collect(),
        }
    }

    pub fn add_keys(&self, set: &KeySet) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for key in &set.keys {
                if state.keys.insert(key.id, *key) != Some(*key) {
                    rows.push(row(Table::Key, key.id.as_bytes().to_vec(), key));
                }
            }
            for wrap in &set.wrapped {
                let slot = (wrap.key, wrap.wrapped_by);
                if state.wraps.get(&slot).is_some_and(|(_, held)| held == wrap) {
                    continue;
                }
                let sequence = state.take();
                let mut key = wrap.key.as_bytes().to_vec();
                key.extend(encode_raw(&wrap.wrapped_by));
                rows.push(row(Table::Wrap, key, &(sequence, wrap.clone())));
                state.wraps.insert(slot, (sequence, wrap.clone()));
            }
        }
        self.write(rows)
    }

    pub fn contacts(&self) -> Vec<(Uuid, [u8; 32])> {
        self.state.lock().unwrap().contacts.clone()
    }

    pub fn set_contacts(&self, mut contacts: Vec<(Uuid, [u8; 32])>) -> Result<(), StoreError> {
        contacts.sort();
        {
            let mut state = self.state.lock().unwrap();
            if state.contacts == contacts {
                return Ok(());
            }
            state.contacts = contacts.clone();
        }
        self.write(vec![row(Table::Contacts, Vec::new(), &contacts)])
    }

    /// The signing keys this device knows `account` by, oldest first.
    pub fn signers(&self, account: Uuid) -> Vec<KnownSigner> {
        self.state
            .lock()
            .unwrap()
            .signers
            .get(&account)
            .cloned()
            .unwrap_or_default()
    }

    pub fn contact_name(&self, account: Uuid) -> Option<String> {
        self.state
            .lock()
            .unwrap()
            .contact_names
            .iter()
            .find(|(named, _)| *named == account)
            .map(|(_, name)| name.clone())
    }

    pub fn set_contact_names(&self, mut names: Vec<(Uuid, String)>) -> Result<(), StoreError> {
        names.sort();
        {
            let mut state = self.state.lock().unwrap();
            if state.contact_names == names {
                return Ok(());
            }
            state.contact_names = names.clone();
        }
        self.write(vec![row(Table::ContactNames, Vec::new(), &names)])
    }

    /// Keeps `key` as one `account` signs with: verified once a contact's card or key change
    /// names it, which no later pin undoes. The newest verified key, last, is the account's current one. A key is
    /// never forgotten, so what an account signed before a new phrase still verifies, though
    /// not as its current key (`Peer::verify_authored`).
    pub fn add_signer(
        &self,
        account: Uuid,
        key: [u8; 32],
        verified: bool,
    ) -> Result<(), StoreError> {
        let known = {
            let mut state = self.state.lock().unwrap();
            let known = state.signers.entry(account).or_default();
            // An existing key never moves: a stale card a server replays must not make an
            // old key the newest, which is the account's current one.
            match known.iter_mut().find(|known| known.key == key) {
                Some(found) if found.verified || !verified => return Ok(()),
                Some(found) => found.verified = true,
                None => known.push(KnownSigner { key, verified }),
            }
            known.clone()
        };
        self.write(vec![row(
            Table::Signers,
            account.as_bytes().to_vec(),
            &known,
        )])
    }

    /// What the host keeps of an open block's local edits no journal holds yet, to give its
    /// replica again after the host stops (`OpenEdits`).
    pub fn open_edits(&self, block: Uuid) -> Option<OpenEdits> {
        self.state.lock().unwrap().open_edits.get(&block).cloned()
    }

    /// Keeps `edits` as `block`'s: kept with none too, for its origin and count, which every
    /// later replica of the block on this device goes on with.
    /// Whether `block`'s local edits were discarded and this account may not edit it: none are
    /// kept for it (`keep_open_edits`).
    pub fn discarded(&self, block: Uuid) -> bool {
        let state = self.state.lock().unwrap();
        state.dropped.contains(&block)
            && !state
                .summaries
                .get(&block)
                .is_some_and(|summary| summary.access.can_edit())
    }

    /// Keeps `edits` as `block`'s, answering whether it did: not for a block whose local edits
    /// were discarded (`drop_local_edits`) while this account may not edit it, as kept they
    /// would keep it from being forgotten. Once it may edit it again, they are kept again.
    pub fn keep_open_edits(&self, block: Uuid, edits: OpenEdits) -> Result<bool, StoreError> {
        let mut state = self.state.lock().unwrap();
        let mut rows = Vec::new();
        if state.dropped.contains(&block) {
            let may_edit = state
                .summaries
                .get(&block)
                .is_some_and(|summary| summary.access.can_edit());
            if !may_edit {
                return Ok(false);
            }
            state.dropped.remove(&block);
            rows.push(Row {
                table: Table::Dropped,
                key: block.as_bytes().to_vec(),
                value: None,
            });
        }
        if state.open_edits.get(&block) != Some(&edits) {
            state.open_edits.insert(block, edits.clone());
            rows.push(row(Table::OpenEdits, block.as_bytes().to_vec(), &edits));
        }
        drop(state);
        self.write(rows)?;
        Ok(true)
    }

    /// Moves `block`'s kept edits aside (`SetAside`), for a replica to open under a fresh origin.
    pub fn set_aside(&self, block: Uuid) -> Result<(), StoreError> {
        let mut state = self.state.lock().unwrap();
        let Some(kept) = state.open_edits.remove(&block) else {
            return Ok(());
        };
        let aside = state.set_aside.entry(block).or_default();
        aside.push(SetAside {
            id: Uuid::new_v4(),
            count: kept.edits.len(),
            kept: Some(kept),
        });
        let over = aside.len().saturating_sub(SetAside::KEPT);
        aside.drain(..over);
        let aside = aside.clone();
        drop(state);
        self.write(vec![
            row(Table::SetAside, block.as_bytes().to_vec(), &aside),
            Row {
                table: Table::OpenEdits,
                key: block.as_bytes().to_vec(),
                value: None,
            },
        ])
    }

    /// Every block's kept edits set aside.
    pub fn set_aside_edits(&self) -> Vec<(Uuid, Vec<SetAside>)> {
        let state = self.state.lock().unwrap();
        state
            .set_aside
            .iter()
            .map(|(block, aside)| (*block, aside.clone()))
            .collect()
    }

    /// Leaves `block`'s set-aside edits `id` a tombstone, once they read and were told.
    pub fn tombstone_set_aside(&self, block: Uuid, id: Uuid) -> Result<(), StoreError> {
        let mut state = self.state.lock().unwrap();
        let Some(aside) = state.set_aside.get_mut(&block) else {
            return Ok(());
        };
        let Some(entry) = aside.iter_mut().find(|entry| entry.id == id) else {
            return Ok(());
        };
        entry.kept = None;
        let aside = aside.clone();
        drop(state);
        self.write(vec![row(
            Table::SetAside,
            block.as_bytes().to_vec(),
            &aside,
        )])
    }

    /// The blocks the search indexer let go (`let_go`).
    pub fn let_go_blocks(&self) -> Vec<(Uuid, Option<ListedBlock>)> {
        let state = self.state.lock().unwrap();
        state
            .let_go
            .iter()
            .map(|(block, listed)| (*block, *listed))
            .collect()
    }

    /// Keeps `block` as one the search indexer let go, with what a catch-up listed of it: a
    /// listing kept before stays when this one has none.
    pub fn let_go(&self, block: Uuid, listed: Option<ListedBlock>) -> Result<(), StoreError> {
        let listed = {
            let mut state = self.state.lock().unwrap();
            let kept = state.let_go.get(&block).copied();
            let listed = listed.or(kept.flatten());
            if kept == Some(listed) {
                return Ok(());
            }
            state.let_go.insert(block, listed);
            listed
        };
        self.write(vec![row(Table::LetGo, block.as_bytes().to_vec(), &listed)])
    }

    /// Forgets `block` as one the search indexer let go: it was read, or is no longer indexed.
    pub fn forget_let_go(&self, block: Uuid) -> Result<(), StoreError> {
        if self.state.lock().unwrap().let_go.remove(&block).is_none() {
            return Ok(());
        }
        self.write(vec![Row {
            table: Table::LetGo,
            key: block.as_bytes().to_vec(),
            value: None,
        }])
    }

    pub fn push(&self, op: Op) -> Result<u64, StoreError> {
        Ok(self.push_all(vec![op])?[0])
    }

    pub fn push_all(&self, ops: Vec<Op>) -> Result<Vec<u64>, StoreError> {
        let mut sequences = Vec::with_capacity(ops.len());
        let rows = {
            let mut state = self.state.lock().unwrap();
            ops.into_iter()
                .map(|op| {
                    let sequence = state.take();
                    let row = row(Table::Outbox, sequence.to_be_bytes().to_vec(), &op);
                    state.outbox.insert(sequence, op);
                    sequences.push(sequence);
                    row
                })
                .collect()
        };
        self.write(rows)?;
        Ok(sequences)
    }

    /// Keeps a sealed commit in the outbox. With `sealing`, the last sequence of the block's
    /// journal the commit holds, the journal notes the seal in the same write.
    pub fn push_publish(&self, pending: Pending) -> Result<u64, StoreError> {
        let (block, head) = (pending.block, pending.commit);
        let op = Op::Publish(pending);
        let (sequence, rows) = {
            let mut state = self.state.lock().unwrap();
            let sequence = state.take();
            let rows = vec![
                row(Table::Outbox, sequence.to_be_bytes().to_vec(), &op),
                row(Table::Head, block.as_bytes().to_vec(), &head),
            ];
            state.outbox.insert(sequence, op);
            state.heads.insert(block, head);
            (sequence, rows)
        };
        self.write(rows)?;
        Ok(sequence)
    }

    /// The block's journal on this device, sealed.
    pub fn journal(&self, block: Uuid) -> Option<Journal> {
        let state = self.state.lock().unwrap();
        let journal = state.journals.get(&block)?;
        Some(Journal {
            tag: journal.head.tag,
            base: journal.head.base.clone(),
            entries: journal.entries.values().cloned().collect(),
            lost: false,
        })
    }

    fn journaled(state: &mut State, block: Uuid, tag: Hash) -> &mut Journaled {
        state.journals.entry(block).or_insert_with(|| Journaled {
            head: JournalHead {
                tag,
                base: JournalBase {
                    commit: None,
                    sequence: 0,
                    state: Vec::new(),
                },
                order: be_block_session::Sequencer::starting(0),
            },
            entries: BTreeMap::new(),
        })
    }

    /// Keeps entries the order numbered (sealed) past the journal's last, starting the journal
    /// over for another `tag`, as the server does.
    pub fn append_journal(
        &self,
        block: Uuid,
        tag: Hash,
        entries: Vec<JournalEntry>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let journal = Self::journaled(&mut state, block, tag);
            if journal.head.tag != tag {
                for sequence in std::mem::take(&mut journal.entries).into_keys() {
                    rows.push(Row {
                        table: Table::JournalEntry,
                        key: journal_entry_key(block, sequence),
                        value: None,
                    });
                }
                journal.head.tag = tag;
            }
            for entry in entries {
                if entry.sequence <= journal.head.order.sequence {
                    continue;
                }
                journal.head.order.sequence = entry.sequence;
                journal.head.order.barrier = journal.head.order.barrier.max(entry.floor);
                rows.push(row(
                    Table::JournalEntry,
                    journal_entry_key(block, entry.sequence),
                    &entry,
                ));
                journal.entries.insert(entry.sequence, entry);
            }
            rows.push(row(
                Table::JournalBase,
                block.as_bytes().to_vec(),
                &journal.head,
            ));
        }
        self.write(rows)
    }

    /// Numbers `submission` (its payload sealed) in this device's own order, as the server
    /// would, for a session kept offline, and keeps it: the entry every replica of the block on
    /// this device is to hear.
    pub fn number_offline(
        &self,
        block: Uuid,
        tag: Hash,
        submission: be_block_session::Submission,
    ) -> Result<JournalEntry, StoreError> {
        // A journal this device starts offline starts on the head it holds.
        if self.journal(block).is_none() {
            let base = JournalBase {
                commit: self.head(block),
                sequence: 0,
                state: Vec::new(),
            };
            self.rebase_journal(block, tag, base)?;
        }
        let entry = {
            let mut state = self.state.lock().unwrap();
            let journal = Self::journaled(&mut state, block, tag);
            let be_block_session::OpId { origin, counter } = submission.id;
            // Numbered before, as the server does: the entry it holds, not a second one.
            if let Some(held) = journal
                .entries
                .values()
                .find(|entry| entry.origin == origin && entry.counter == counter)
            {
                return Ok(held.clone());
            }
            // Numbered on a copy: `append_journal` keeps the entry and moves the order past it.
            let mut order = journal.head.order;
            let op = order.number(submission, None);
            JournalEntry {
                sequence: op.sequence,
                origin,
                counter,
                base: op.base,
                floor: op.floor,
                received: None,
                payload: op.payload,
            }
        };
        self.append_journal(block, tag, vec![entry.clone()])?;
        Ok(entry)
    }

    /// Starts the journal from `base` (sealed), as the server does: keeps the entries after it,
    /// and for another `tag` drops every entry. A base that goes back while nothing after the
    /// journal's own base was numbered starts the order afresh from it.
    pub fn rebase_journal(
        &self,
        block: Uuid,
        tag: Hash,
        base: JournalBase,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let journal = Self::journaled(&mut state, block, tag);
            let other = journal.head.tag != tag;
            let order = journal.head.order;
            let goes_on = !other
                && (order.sequence > base.sequence.max(journal.head.base.sequence)
                    || base.sequence >= journal.head.base.sequence);
            if !goes_on {
                journal.head.order = be_block_session::Sequencer::starting(base.sequence);
            }
            let cut = match other {
                true => u64::MAX,
                false => base.sequence,
            };
            let dropped: Vec<u64> = journal.entries.range(..=cut).map(|(at, _)| *at).collect();
            for sequence in dropped {
                journal.entries.remove(&sequence);
                rows.push(Row {
                    table: Table::JournalEntry,
                    key: journal_entry_key(block, sequence),
                    value: None,
                });
            }
            journal.head.tag = tag;
            journal.head.base = base;
            rows.push(row(
                Table::JournalBase,
                block.as_bytes().to_vec(),
                &journal.head,
            ));
        }
        self.write(rows)
    }

    /// Makes this device's journal `journal` (sealed), as the server holds it: its base, its
    /// entries and its order, whatever this device held before.
    pub fn replace_journal(&self, block: Uuid, journal: Journal) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let held = Self::journaled(&mut state, block, journal.tag);
            for sequence in std::mem::take(&mut held.entries).into_keys() {
                rows.push(Row {
                    table: Table::JournalEntry,
                    key: journal_entry_key(block, sequence),
                    value: None,
                });
            }
            let mut order = be_block_session::Sequencer::starting(journal.base.sequence);
            for entry in &journal.entries {
                order.sequence = order.sequence.max(entry.sequence);
                order.barrier = order.barrier.max(entry.floor);
            }
            held.head = JournalHead {
                tag: journal.tag,
                base: journal.base,
                order,
            };
            for entry in journal.entries {
                rows.push(row(
                    Table::JournalEntry,
                    journal_entry_key(block, entry.sequence),
                    &entry,
                ));
                held.entries.insert(entry.sequence, entry);
            }
            rows.push(row(
                Table::JournalBase,
                block.as_bytes().to_vec(),
                &held.head,
            ));
        }
        self.write(rows)
    }

    pub fn creating(&self, block: Uuid) -> bool {
        let state = self.state.lock().unwrap();
        state
            .outbox
            .values()
            .any(|op| matches!(op, Op::Create(creation) if creation.block == block))
    }

    /// Whether the block's creation was set aside among the unsent edits.
    pub fn creation_set_aside(&self, block: Uuid) -> bool {
        let state = self.state.lock().unwrap();
        state.unsent.get(&block).is_some_and(|unsent| {
            unsent
                .ops
                .iter()
                .any(|op| matches!(op, Op::Create(creation) if creation.block == block))
        })
    }

    pub fn outbox(&self) -> Vec<(u64, Op)> {
        let state = self.state.lock().unwrap();
        state
            .outbox
            .iter()
            .map(|(sequence, op)| (*sequence, op.clone()))
            .collect()
    }

    pub fn pending_for(&self, block: Uuid) -> Vec<(u64, Op)> {
        let state = self.state.lock().unwrap();
        state
            .outbox
            .iter()
            .filter(|(_, op)| op.block() == block)
            .map(|(sequence, op)| (*sequence, op.clone()))
            .collect()
    }

    pub fn holds(&self, sequence: u64) -> bool {
        self.state.lock().unwrap().outbox.contains_key(&sequence)
    }

    pub fn has_structural(&self, block: Uuid) -> bool {
        let state = self.state.lock().unwrap();
        state
            .outbox
            .values()
            .any(|op| op.block() == block && !matches!(op, Op::Publish(_) | Op::Hold(_)))
    }

    pub fn below(&self, root: Uuid) -> HashSet<Uuid> {
        let parents: HashMap<Uuid, Uuid> = self
            .summaries()
            .into_iter()
            .filter_map(|summary| Some((summary.id, summary.parent?.block()?)))
            .collect();
        parents
            .keys()
            .copied()
            .filter(|block| {
                let mut current = *block;
                for _ in 0..=parents.len() {
                    match parents.get(&current) {
                        Some(parent) if *parent == root => return true,
                        Some(parent) => current = *parent,
                        None => return false,
                    }
                }
                false
            })
            .collect()
    }

    pub fn forget(&self, blocks: &HashSet<Uuid>) -> Result<usize, StoreError> {
        let mut rows = Vec::new();
        let forgotten = {
            let mut guard = self.state.lock().unwrap();
            let state = &mut *guard;
            let kept: HashSet<Uuid> = state
                .outbox
                .values()
                .map(Op::block)
                .chain(state.unsent.keys().copied())
                .chain(
                    state
                        .open_edits
                        .iter()
                        .filter(|(_, kept)| !kept.edits.is_empty())
                        .map(|(block, _)| *block),
                )
                .chain(
                    state
                        .journals
                        .iter()
                        .filter(|(_, journal)| !journal.entries.is_empty())
                        .map(|(block, _)| *block),
                )
                .collect();
            let gone: HashSet<Uuid> = blocks.difference(&kept).copied().collect();
            let about = |listing: &Listing| match listing {
                Listing::Children(BlockParent::Block(block))
                | Listing::Backrefs(block)
                | Listing::Block(block)
                | Listing::References(block)
                | Listing::OpenSettings(block) => gone.contains(block),
                _ => false,
            };
            for (listing, held) in state.listings.iter_mut() {
                let before = held.len();
                match about(listing) {
                    true => held.clear(),
                    false => held.retain(|id| !gone.contains(id)),
                }
                if about(listing) {
                    rows.push(Row {
                        table: Table::Listing,
                        key: encode_raw(listing),
                        value: None,
                    });
                } else if held.len() != before {
                    let at = state.listed_at.get(listing).copied().unwrap_or_default();
                    rows.push(row(
                        Table::Listing,
                        encode_raw(listing),
                        &(at, &held.clone()),
                    ));
                }
            }
            for block in &gone {
                if state.drop_summary(*block) {
                    rows.push(Row {
                        table: Table::Summary,
                        key: block.as_bytes().to_vec(),
                        value: None,
                    });
                }
            }
            state.listings.retain(|listing, _| !about(listing));
            state.listed_at.retain(|listing, _| !about(listing));
            for block in &gone {
                announcements::forget(state, *block, &mut rows);
                if state.journals.remove(block).is_some() {
                    rows.push(Row {
                        table: Table::JournalBase,
                        key: block.as_bytes().to_vec(),
                        value: None,
                    });
                }
                for (table, gone_too) in [
                    (Table::OpenEdits, state.open_edits.remove(block).is_some()),
                    (Table::SetAside, state.set_aside.remove(block).is_some()),
                    (Table::Dropped, state.dropped.remove(block)),
                ] {
                    if gone_too {
                        rows.push(Row {
                            table,
                            key: block.as_bytes().to_vec(),
                            value: None,
                        });
                    }
                }
                for (table, held) in [
                    (Table::Head, &mut state.heads),
                    (Table::Remote, &mut state.remotes),
                    (Table::Held, &mut state.held),
                ] {
                    if held.remove(block).is_some() {
                        rows.push(Row {
                            table,
                            key: block.as_bytes().to_vec(),
                            value: None,
                        });
                    }
                }
            }
            gone.len()
        };
        self.write(rows)?;
        Ok(forgotten)
    }

    pub fn has_pending(&self, block: Uuid) -> bool {
        let state = self.state.lock().unwrap();
        state.outbox.values().any(|op| op.block() == block)
    }

    pub fn remove(&self, sequences: &[u64]) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for sequence in sequences {
                if state.outbox.remove(sequence).is_some() {
                    rows.push(Row {
                        table: Table::Outbox,
                        key: sequence.to_be_bytes().to_vec(),
                        value: None,
                    });
                }
            }
        }
        self.write(rows)
    }

    pub fn counts(&self) -> PendingCounts {
        let state = self.state.lock().unwrap();
        let mut blocks = HashSet::new();
        let mut commits = 0;
        for op in state.outbox.values() {
            if let Op::Publish(pending) = op {
                commits += 1;
                blocks.insert(pending.block);
            }
        }
        PendingCounts {
            operations: state.outbox.len(),
            commits,
            blocks: blocks.len(),
            unsent: state.unsent.len(),
        }
    }

    pub fn move_to_unsent(
        &self,
        block: Uuid,
        since: i64,
        refused: Option<ErrorCode>,
    ) -> Result<bool, StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let moved: Vec<u64> = state
                .outbox
                .iter()
                .filter(|(_, op)| op.block() == block)
                .map(|(sequence, _)| *sequence)
                .collect();
            if moved.is_empty() {
                return Ok(false);
            }
            let head = state.heads.get(&block).copied();
            let mut ops = Vec::new();
            for sequence in moved {
                ops.extend(state.outbox.remove(&sequence));
                rows.push(Row {
                    table: Table::Outbox,
                    key: sequence.to_be_bytes().to_vec(),
                    value: None,
                });
            }
            let unsent = state.unsent.entry(block).or_insert(Unsent {
                head,
                since,
                ops: Vec::new(),
                refused: None,
            });
            unsent.head = head.or(unsent.head);
            unsent.ops.extend(ops);
            // A later setting aside (what was made of it since) keeps why it was first refused.
            unsent.refused = refused.or(unsent.refused.take());
            rows.push(row(Table::Unsent, block.as_bytes().to_vec(), &*unsent));
        }
        self.write(rows)?;
        Ok(true)
    }

    pub fn is_unsent(&self, block: Uuid) -> bool {
        self.state.lock().unwrap().unsent.contains_key(&block)
    }

    /// The code the server refused `block` with, when what this device made of it was set aside
    /// for that (`Unsent::refused`).
    pub fn refused(&self, block: Uuid) -> Option<ErrorCode> {
        self.state
            .lock()
            .unwrap()
            .unsent
            .get(&block)
            .and_then(|unsent| unsent.refused)
    }

    pub fn unsent(&self) -> Vec<(Uuid, Unsent)> {
        let state = self.state.lock().unwrap();
        state
            .unsent
            .iter()
            .map(|(block, unsent)| (*block, unsent.clone()))
            .collect()
    }

    pub fn restore_unsent(&self, block: Uuid) -> Result<bool, StoreError> {
        let rows = {
            let mut state = self.state.lock().unwrap();
            let Some(unsent) = state.unsent.remove(&block) else {
                return Ok(false);
            };
            let mut rows = vec![Row {
                table: Table::Unsent,
                key: block.as_bytes().to_vec(),
                value: None,
            }];
            for op in unsent.ops {
                let sequence = state.take();
                rows.push(row(Table::Outbox, sequence.to_be_bytes().to_vec(), &op));
                state.outbox.insert(sequence, op);
            }
            if let Some(head) = unsent.head {
                rows.push(row(Table::Head, block.as_bytes().to_vec(), &head));
                state.heads.insert(block, head);
            }
            rows
        };
        self.write(rows)?;
        Ok(true)
    }

    /// Drops the local edits this device keeps of `block` beside its outbox: its kept edits
    /// (`OpenEdits`) and its journal. For a block this account lost access to, which they can
    /// never reach and which `forget` would otherwise keep for them.
    pub fn drop_local_edits(&self, block: Uuid) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            state.dropped.insert(block);
            rows.push(row(Table::Dropped, block.as_bytes().to_vec(), &()));
            if state.open_edits.remove(&block).is_some() {
                rows.push(Row {
                    table: Table::OpenEdits,
                    key: block.as_bytes().to_vec(),
                    value: None,
                });
            }
            if let Some(journal) = state.journals.remove(&block) {
                for sequence in journal.entries.into_keys() {
                    rows.push(Row {
                        table: Table::JournalEntry,
                        key: journal_entry_key(block, sequence),
                        value: None,
                    });
                }
                rows.push(Row {
                    table: Table::JournalBase,
                    key: block.as_bytes().to_vec(),
                    value: None,
                });
            }
        }
        self.write(rows)
    }

    pub fn take_unsent(&self, block: Uuid) -> Result<Option<Unsent>, StoreError> {
        let Some(unsent) = self.state.lock().unwrap().unsent.remove(&block) else {
            return Ok(None);
        };
        self.write(vec![Row {
            table: Table::Unsent,
            key: block.as_bytes().to_vec(),
            value: None,
        }])?;
        Ok(Some(unsent))
    }
}

fn row<T: Serialize>(table: Table, key: Vec<u8>, value: &T) -> Row {
    Row {
        table,
        key,
        value: Some(encode(value)),
    }
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = vec![VERSION];
    bytes.extend(encode_raw(value));
    bytes
}

fn encode_raw<T: Serialize>(value: &T) -> Vec<u8> {
    be_block_protocol::encode(value).expect("a refs record always encodes")
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    match bytes.split_first() {
        Some((&VERSION, rest)) => decode_raw(rest),
        _ => Err(StoreError::Corrupt),
    }
}

fn decode_raw<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    be_block_protocol::decode(bytes).map_err(|_| StoreError::Corrupt)
}

fn uuid_key(bytes: &[u8]) -> Result<Uuid, StoreError> {
    Uuid::from_slice(bytes).map_err(|_| StoreError::Corrupt)
}

fn sequence_key(bytes: &[u8]) -> Result<u64, StoreError> {
    Ok(u64::from_be_bytes(
        bytes.try_into().map_err(|_| StoreError::Corrupt)?,
    ))
}

fn newest(state: &State) -> u64 {
    state
        .summaries
        .values()
        .map(|summary| summary.revision)
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
