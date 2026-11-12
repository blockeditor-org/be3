use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_block_commit::CommitId;
use be_block_graph::BlockParent;
use be_block_protocol::{
    BlockSummary, EscrowKey, Journal, JournalBase, JournalEntry, KeyRow, KeySet, Listing,
    Organization, OrganizationKind, OrganizationRole, WrappedBy, WrappedKey,
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
}

impl Table {
    pub const ALL: [Self; 16] = [
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
    pub pinned: Option<EscrowKey>,
    #[serde(default)]
    pub server: Option<be_block_protocol::ServerAccount>,
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
    #[serde(default)]
    pub references: Vec<Uuid>,
    pub key: Uuid,
    pub keys: KeySet,
    pub owner_only: bool,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Op {
    Create(Creation),
    Publish(Pending),
    Move(Moving),
    Define(Defining),
}

impl Op {
    pub fn block(&self) -> Uuid {
        match self {
            Self::Create(creation) => creation.block,
            Self::Publish(pending) => pending.block,
            Self::Move(moving) => moving.block,
            Self::Define(defining) => defining.block,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Unsent {
    pub head: Option<CommitId>,
    pub since: i64,
    pub ops: Vec<Op>,
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PendingCounts {
    pub operations: usize,
    pub commits: usize,
    pub blocks: usize,
    pub unsent: usize,
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
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<(Uuid, WrappedBy), (u64, WrappedKey)>,
    contacts: Vec<(Uuid, [u8; 32])>,
    signers: HashMap<Uuid, Vec<KnownSigner>>,
    outbox: BTreeMap<u64, Op>,
    unsent: BTreeMap<Uuid, Unsent>,
    journals: HashMap<Uuid, Journaled>,
    /// What edits made here announce, by block and key, until sent and for a while after.
    announcements: HashMap<Uuid, BTreeMap<String, Waiting>>,
    next: u64,
}

/// A block's journal on this device, sealed as on the server: what this device's replica
/// sequenced as its owner since the base, kept until a commit holds it.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct JournalHead {
    tag: Hash,
    generation: u64,
    base: JournalBase,
    /// A seal this device made of the journal, with the last sequence it holds, written with
    /// the seal's commit in one write and dropped by the next rebase: while it stands, the
    /// seal's commit holds the journal through that sequence (`Peer::sealing`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sealing: Option<(CommitId, u64)>,
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
        state
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
            Table::Listing => {
                self.listings.insert(decode_raw(&row.key)?, decode(value)?);
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
                            generation: 0,
                            base: JournalBase {
                                commit: None,
                                sequence: 0,
                                state: Vec::new(),
                            },
                            sealing: None,
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

    pub fn set_remote(&self, block: Uuid, head: CommitId) -> Result<(), StoreError> {
        if self.state.lock().unwrap().remotes.insert(block, head) == Some(head) {
            return Ok(());
        }
        self.write(vec![row(Table::Remote, block.as_bytes().to_vec(), &head)])
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

    pub fn observe(&self, summaries: &[BlockSummary]) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for summary in summaries {
                let Some(head) = summary.head else { continue };
                if state.remotes.insert(summary.id, head) != Some(head) {
                    rows.push(row(Table::Remote, summary.id.as_bytes().to_vec(), &head));
                }
            }
        }
        self.write(rows)
    }

    /// Keeps a summary unless this device already has a newer one of the block.
    fn put(state: &mut State, summary: &BlockSummary, rows: &mut Vec<Row>) {
        let newer = state
            .summaries
            .get(&summary.id)
            .is_some_and(|known| known.revision > summary.revision);
        if newer || state.summaries.get(&summary.id) == Some(summary) {
            return;
        }
        state.summaries.insert(summary.id, summary.clone());
        rows.push(row(Table::Summary, summary.id.as_bytes().to_vec(), summary));
    }

    fn put_listing(state: &mut State, listing: Listing, ids: Vec<Uuid>, rows: &mut Vec<Row>) {
        if state.listings.get(&listing) == Some(&ids) {
            return;
        }
        rows.push(row(Table::Listing, encode_raw(&listing), &ids));
        state.listings.insert(listing, ids);
    }

    pub fn set_listing(&self, listing: Listing, blocks: &[BlockSummary]) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            for summary in blocks {
                Self::put(&mut state, summary, &mut rows);
            }
            let ids = blocks.iter().map(|summary| summary.id).collect();
            Self::put_listing(&mut state, listing, ids, &mut rows);
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
            state.summaries.insert(block, summary.clone());
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

    /// A listing as this device last knew it, for answering offline. What a block references is
    /// read from its summary, since one pushed or revised after the listing was fetched (another
    /// device's seal, or this device's own offline) names what it references then.
    /// Children and deleted blocks are those whose summary still puts them there, so a block
    /// moved since shows only where it went, and backrefs are the summaries that reference the
    /// block.
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
            Listing::Children(parent) => Some(
                self.listing(&listing)?
                    .into_iter()
                    .filter(|summary| summary.parent == Some(parent))
                    .collect(),
            ),
            Listing::Deleted => Some(
                self.listing(&listing)?
                    .into_iter()
                    .filter(|summary| summary.parent == Some(BlockParent::Detached))
                    .collect(),
            ),
            _ => self.listing(&listing),
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
    pub fn push_publish(&self, pending: Pending, sealing: Option<u64>) -> Result<u64, StoreError> {
        let (block, head) = (pending.block, pending.commit);
        let op = Op::Publish(pending);
        let (sequence, rows) = {
            let mut state = self.state.lock().unwrap();
            let sequence = state.take();
            let mut rows = vec![
                row(Table::Outbox, sequence.to_be_bytes().to_vec(), &op),
                row(Table::Head, block.as_bytes().to_vec(), &head),
            ];
            if let Some(through) = sealing
                && let Some(journal) = state.journals.get_mut(&block)
            {
                journal.head.sealing = Some((head, through));
                rows.push(row(
                    Table::JournalBase,
                    block.as_bytes().to_vec(),
                    &journal.head,
                ));
            }
            state.outbox.insert(sequence, op);
            state.heads.insert(block, head);
            (sequence, rows)
        };
        self.write(rows)?;
        Ok(sequence)
    }

    /// The seal this device made of the block's journal since its last rebase, and the last
    /// sequence it holds (`JournalHead::sealing`).
    pub fn sealing(&self, block: Uuid) -> Option<(CommitId, u64)> {
        let state = self.state.lock().unwrap();
        state.journals.get(&block)?.head.sealing
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

    /// Appends sealed entries past the journal's last, starting the journal over for another
    /// `tag`, as the server does.
    pub fn append_journal(
        &self,
        block: Uuid,
        tag: Hash,
        generation: u64,
        entries: Vec<JournalEntry>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            let journal = state.journals.entry(block).or_insert_with(|| Journaled {
                head: JournalHead {
                    tag,
                    generation,
                    base: JournalBase {
                        commit: None,
                        sequence: 0,
                        state: Vec::new(),
                    },
                    sealing: None,
                },
                entries: BTreeMap::new(),
            });
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
            journal.head.generation = generation;
            rows.push(row(
                Table::JournalBase,
                block.as_bytes().to_vec(),
                &journal.head,
            ));
            let mut last = journal
                .entries
                .last_key_value()
                .map_or(journal.head.base.sequence, |(sequence, _)| *sequence);
            for entry in entries {
                if entry.sequence <= last {
                    continue;
                }
                last = entry.sequence;
                rows.push(row(
                    Table::JournalEntry,
                    journal_entry_key(block, entry.sequence),
                    &entry,
                ));
                journal.entries.insert(entry.sequence, entry);
            }
        }
        self.write(rows)
    }

    /// Starts the journal over from `base` (sealed), dropping every entry, as the server does,
    /// and keeps `entries` past it (sealed) in the same write.
    pub fn rebase_journal(
        &self,
        block: Uuid,
        tag: Hash,
        generation: u64,
        base: JournalBase,
        entries: Vec<JournalEntry>,
    ) -> Result<(), StoreError> {
        let mut rows = Vec::new();
        {
            let mut state = self.state.lock().unwrap();
            // A rebase starts the journal over from a commit: no seal of the old one stands.
            let head = JournalHead {
                tag,
                generation,
                base,
                sealing: None,
            };
            if let Some(journal) = state.journals.get(&block) {
                for sequence in journal.entries.keys() {
                    rows.push(Row {
                        table: Table::JournalEntry,
                        key: journal_entry_key(block, *sequence),
                        value: None,
                    });
                }
            }
            rows.push(row(Table::JournalBase, block.as_bytes().to_vec(), &head));
            let mut kept = BTreeMap::new();
            for entry in entries {
                if entry.sequence <= head.base.sequence {
                    continue;
                }
                rows.push(row(
                    Table::JournalEntry,
                    journal_entry_key(block, entry.sequence),
                    &entry,
                ));
                kept.insert(entry.sequence, entry);
            }
            state.journals.insert(
                block,
                Journaled {
                    head,
                    entries: kept,
                },
            );
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
            .any(|op| op.block() == block && !matches!(op, Op::Publish(_)))
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
                    rows.push(row(Table::Listing, encode_raw(listing), &held.clone()));
                }
            }
            for block in &gone {
                if state.summaries.remove(block).is_some() {
                    rows.push(Row {
                        table: Table::Summary,
                        key: block.as_bytes().to_vec(),
                        value: None,
                    });
                }
            }
            state.listings.retain(|listing, _| !about(listing));
            for block in &gone {
                announcements::forget(state, *block, &mut rows);
                if state.journals.remove(block).is_some() {
                    rows.push(Row {
                        table: Table::JournalBase,
                        key: block.as_bytes().to_vec(),
                        value: None,
                    });
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

    pub fn move_to_unsent(&self, block: Uuid, since: i64) -> Result<bool, StoreError> {
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
            });
            unsent.head = head.or(unsent.head);
            unsent.ops.extend(ops);
            rows.push(row(Table::Unsent, block.as_bytes().to_vec(), &*unsent));
        }
        self.write(rows)?;
        Ok(true)
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
