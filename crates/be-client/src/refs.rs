use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use be_commit::CommitId;
use be_graph::BlockParent;
use be_protocol::{
    BlockSummary, EscrowKey, KeyRow, KeySet, Listing, Organization, OrganizationKind,
    OrganizationRole, WrappedBy, WrappedKey,
};
use be_store::{Hash, StoreError};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
mod memory;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;

#[cfg(target_arch = "wasm32")]
type Backend = memory::MemoryRefs;
#[cfg(not(target_arch = "wasm32"))]
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
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Table {
    pub const ALL: [Self; 10] = [
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn memory() -> Self {
        Self {
            backend: Arc::new(Backend::in_memory().expect("an in-memory database always opens")),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn memory() -> Self {
        Self {
            backend: Arc::new(Backend::default()),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
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
        let state = State::load(rows)?;
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pending {
    pub block: Uuid,
    pub commit: CommitId,
    pub expected: Option<CommitId>,
    pub parents: Vec<CommitId>,
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
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Moving {
    pub block: Uuid,
    pub parent: BlockParent,
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

#[derive(Default)]
struct State {
    session: Option<Session>,
    heads: HashMap<Uuid, CommitId>,
    remotes: HashMap<Uuid, CommitId>,
    listings: HashMap<Listing, Vec<BlockSummary>>,
    keys: HashMap<Uuid, KeyRow>,
    wraps: HashMap<(Uuid, WrappedBy), (u64, WrappedKey)>,
    contacts: Vec<(Uuid, [u8; 32])>,
    outbox: BTreeMap<u64, Op>,
    unsent: BTreeMap<Uuid, Unsent>,
    next: u64,
}

impl State {
    fn load(rows: Vec<Row>) -> Result<Self, StoreError> {
        let mut state = Self::default();
        for row in rows {
            let Some(value) = row.value else { continue };
            match row.table {
                Table::Session => state.session = Some(decode(&value)?),
                Table::Head => {
                    state.heads.insert(uuid_key(&row.key)?, decode(&value)?);
                }
                Table::Remote => {
                    state.remotes.insert(uuid_key(&row.key)?, decode(&value)?);
                }
                Table::Listing => {
                    state
                        .listings
                        .insert(decode_raw(&row.key)?, decode(&value)?);
                }
                Table::Key => {
                    let key: KeyRow = decode(&value)?;
                    state.keys.insert(key.id, key);
                }
                Table::Wrap => {
                    let (sequence, wrap): (u64, WrappedKey) = decode(&value)?;
                    state.next = state.next.max(sequence + 1);
                    state
                        .wraps
                        .insert((wrap.key, wrap.wrapped_by), (sequence, wrap));
                }
                Table::Contacts => state.contacts = decode(&value)?,
                Table::Outbox => {
                    let sequence = sequence_key(&row.key)?;
                    state.next = state.next.max(sequence + 1);
                    state.outbox.insert(sequence, decode(&value)?);
                }
                Table::Unsent => {
                    state.unsent.insert(uuid_key(&row.key)?, decode(&value)?);
                }
                Table::Workspaces => {}
            }
        }
        Ok(state)
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

    pub fn set_listing(&self, listing: Listing, blocks: &[BlockSummary]) -> Result<(), StoreError> {
        {
            let mut state = self.state.lock().unwrap();
            if state.listings.get(&listing).map(Vec::as_slice) == Some(blocks) {
                return Ok(());
            }
            state.listings.insert(listing, blocks.to_vec());
        }
        self.write(vec![row(
            Table::Listing,
            encode_raw(&listing),
            &blocks.to_vec(),
        )])
    }

    pub fn add_to_listing(
        &self,
        listing: Listing,
        summary: &BlockSummary,
    ) -> Result<(), StoreError> {
        let mut blocks = self.listing(&listing).unwrap_or_default();
        blocks.retain(|held| held.id != summary.id);
        blocks.push(summary.clone());
        self.set_listing(listing, &blocks)
    }

    pub fn revise(&self, mut summary: BlockSummary) -> Result<(), StoreError> {
        let block = summary.id;
        let parent = summary.parent;
        let listings: Vec<Listing> = {
            let state = self.state.lock().unwrap();
            summary.revision = state
                .listings
                .values()
                .flatten()
                .map(|held| held.revision)
                .max()
                .unwrap_or_default()
                .max(summary.revision);
            state
                .listings
                .iter()
                .filter(|(_, blocks)| blocks.iter().any(|held| held.id == block))
                .map(|(listing, _)| *listing)
                .collect()
        };
        for listing in listings {
            let mut blocks = self.listing(&listing).unwrap_or_default();
            let leaves = match listing {
                Listing::Children(under) => Some(under) != parent,
                Listing::Deleted => parent != Some(BlockParent::Detached),
                _ => false,
            };
            match leaves {
                true => blocks.retain(|held| held.id != block),
                false => {
                    for held in blocks.iter_mut().filter(|held| held.id == block) {
                        *held = summary.clone();
                    }
                }
            }
            self.set_listing(listing, &blocks)?;
        }
        self.set_listing(Listing::Block(block), std::slice::from_ref(&summary))?;
        let mut joins = Vec::new();
        if let Some(parent) = parent {
            joins.push(Listing::Children(parent));
            if parent == BlockParent::Detached {
                joins.push(Listing::Deleted);
            }
        }
        for listing in joins {
            if self.listing(&listing).is_some() {
                self.add_to_listing(listing, &summary)?;
            }
        }
        Ok(())
    }

    pub fn lose_access(&self, block: Uuid) -> Result<(), StoreError> {
        let Some(mut summary) = self.summary(block) else {
            return Ok(());
        };
        summary.access = be_graph::Access::None;
        self.set_listing(Listing::Block(block), &[summary])
    }

    pub fn listing(&self, listing: &Listing) -> Option<Vec<BlockSummary>> {
        self.state.lock().unwrap().listings.get(listing).cloned()
    }

    pub fn newest_revision(&self) -> u64 {
        let state = self.state.lock().unwrap();
        state
            .listings
            .values()
            .flatten()
            .map(|summary| summary.revision)
            .max()
            .unwrap_or_default()
    }

    pub fn current(&self, mut blocks: Vec<BlockSummary>) -> Vec<BlockSummary> {
        let newest = self.newest_revision();
        for summary in &mut blocks {
            summary.revision = newest;
        }
        blocks
    }

    pub fn summary(&self, block: Uuid) -> Option<BlockSummary> {
        let state = self.state.lock().unwrap();
        let own = state
            .listings
            .get(&Listing::Block(block))
            .and_then(|blocks| blocks.first());
        state
            .listings
            .values()
            .flatten()
            .filter(|summary| summary.id == block)
            .fold(own, |best, summary| match best {
                Some(best) if best.revision >= summary.revision => Some(best),
                _ => Some(summary),
            })
            .cloned()
    }

    pub fn summaries(&self) -> Vec<BlockSummary> {
        let state = self.state.lock().unwrap();
        let mut seen = HashSet::new();
        state
            .listings
            .values()
            .flatten()
            .filter(|summary| seen.insert(summary.id))
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

    pub fn push(&self, op: Op) -> Result<u64, StoreError> {
        let (sequence, row) = {
            let mut state = self.state.lock().unwrap();
            let sequence = state.take();
            let row = row(Table::Outbox, sequence.to_be_bytes().to_vec(), &op);
            state.outbox.insert(sequence, op);
            (sequence, row)
        };
        self.write(vec![row])?;
        Ok(sequence)
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
    be_protocol::encode(value).expect("a refs record always encodes")
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    match bytes.split_first() {
        Some((&VERSION, rest)) => decode_raw(rest),
        _ => Err(StoreError::Corrupt),
    }
}

fn decode_raw<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StoreError> {
    be_protocol::decode(bytes).map_err(|_| StoreError::Corrupt)
}

fn uuid_key(bytes: &[u8]) -> Result<Uuid, StoreError> {
    Uuid::from_slice(bytes).map_err(|_| StoreError::Corrupt)
}

fn sequence_key(bytes: &[u8]) -> Result<u64, StoreError> {
    Ok(u64::from_be_bytes(
        bytes.try_into().map_err(|_| StoreError::Corrupt)?,
    ))
}
