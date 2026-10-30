use reactive::Get;
use reactive::Set;
use reactive::{Pending, Ready, wait_while};
use stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use be_block::{BlockMetadata, Definition, Entries};
use block_plugin_api::{AccessLevel, BlockLocation};
use reactive::OwnedSignal;
use uuid::Uuid;

use crate::host::Waker;

const RETIRED: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BlockParent {
    Detached,
    Root,
    Block(Uuid),
}

impl BlockParent {
    pub const fn block(self) -> Option<Uuid> {
        match self {
            Self::Block(id) => Some(id),
            Self::Detached | Self::Root => None,
        }
    }

    pub fn encode(self) -> BlockLocation {
        match self {
            Self::Root => BlockLocation::Root,
            Self::Detached => BlockLocation::Detached,
            Self::Block(id) => BlockLocation::Block(id.into_bytes()),
        }
    }

    pub fn decode(location: BlockLocation) -> Self {
        match location {
            BlockLocation::Root => Self::Root,
            BlockLocation::Detached => Self::Detached,
            BlockLocation::Block(id) => Self::Block(Uuid::from_bytes(id)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BlockQuery {
    Roots,
    Detached,
    Shared,
    Children(Uuid),
    References(Uuid),
    Backrefs(Uuid),
    Parents(Uuid),
    Block(Uuid),
    OpenSettings(Uuid),
}

impl BlockQuery {
    pub fn encode(self) -> block_plugin_api::BlockQuery {
        use block_plugin_api::BlockQuery as Wire;
        match self {
            Self::Roots => Wire::Roots,
            Self::Detached => Wire::Detached,
            Self::Shared => Wire::Shared,
            Self::Children(id) => Wire::Children(id.into_bytes()),
            Self::References(id) => Wire::References(id.into_bytes()),
            Self::Backrefs(id) => Wire::Backrefs(id.into_bytes()),
            Self::Parents(id) => Wire::Parents(id.into_bytes()),
            Self::Block(id) => Wire::Block(id.into_bytes()),
            Self::OpenSettings(id) => Wire::OpenSettings(id.into_bytes()),
        }
    }

    pub fn decode(query: block_plugin_api::BlockQuery) -> Self {
        use block_plugin_api::BlockQuery as Wire;
        match query {
            Wire::Roots => Self::Roots,
            Wire::Detached => Self::Detached,
            Wire::Shared => Self::Shared,
            Wire::Children(id) => Self::Children(Uuid::from_bytes(id)),
            Wire::References(id) => Self::References(Uuid::from_bytes(id)),
            Wire::Backrefs(id) => Self::Backrefs(Uuid::from_bytes(id)),
            Wire::Parents(id) => Self::Parents(Uuid::from_bytes(id)),
            Wire::Block(id) => Self::Block(Uuid::from_bytes(id)),
            Wire::OpenSettings(id) => Self::OpenSettings(Uuid::from_bytes(id)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInfo {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Option<Uuid>,
    pub parent: Option<BlockParent>,
    pub references: Vec<Uuid>,
    pub access: AccessLevel,
    pub metadata: BlockMetadata,
    pub locked: bool,
    pub deleted: bool,
    pub sets_access: bool,
}

impl BlockInfo {
    pub fn new(id: Uuid, block_type: Uuid, parent: BlockParent) -> Self {
        Self {
            id,
            block_type,
            author: Some(Uuid::nil()),
            parent: Some(parent),
            references: Vec::new(),
            access: AccessLevel::Manage,
            metadata: BlockMetadata::default(),
            locked: false,
            deleted: false,
            sets_access: false,
        }
    }

    pub fn name(&self) -> Option<String> {
        self.metadata.name()
    }

    pub fn file_name(&self) -> Option<String> {
        self.metadata
            .get(&be_block::INTRINSIC_NAME)
            .filter(|name| !name.is_empty())
            .or_else(|| self.name())
    }

    pub fn no_access(&self) -> bool {
        !self.access.can_view()
    }

    pub fn reach(&self) -> block_ui::Reach {
        match (self.no_access(), self.locked) {
            (true, _) => block_ui::Reach::NoAccess,
            (false, true) => block_ui::Reach::Locked,
            (false, false) => block_ui::Reach::Open,
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.metadata.insert(&be_block::NAME, &name.into());
    }

    pub fn set_intrinsic_name(&mut self, name: impl Into<String>) {
        self.metadata
            .insert(&be_block::INTRINSIC_NAME, &name.into());
    }

    pub fn set_thumbhash(&mut self, thumbhash: &be_block::Thumbhash) {
        self.metadata.insert(&be_block::THUMBHASH, thumbhash);
    }

    pub fn named_by_hand(&self) -> bool {
        self.metadata.named_by_hand()
    }

    pub fn artifact(&self) -> Option<be_block::Artifact> {
        self.metadata.get(&be_block::ARTIFACT)
    }

    pub fn thumbhash(&self) -> Option<be_block::Thumbhash> {
        self.metadata.get(&be_block::THUMBHASH)
    }

    pub fn is_artifact(&self) -> bool {
        self.metadata.contains(&be_block::ARTIFACT)
    }

    pub fn label(&self, types: &dyn block_ui::BlockTypes) -> block_ui::BlockLabel {
        block_ui::BlockLabel::new(
            types,
            self.block_type,
            self.name().as_deref(),
            self.named_by_hand(),
        )
        .reached(self.reach())
        .deleted(self.deleted)
    }

    pub fn encode(&self) -> block_plugin_api::BlockInfo {
        block_plugin_api::BlockInfo {
            block_id: self.id.into_bytes(),
            block_type: self.block_type.into_bytes(),
            author: self.author.map(Uuid::into_bytes),
            parent: self.parent.map(BlockParent::encode),
            references: self.references.iter().map(|id| id.into_bytes()).collect(),
            access: self.access,
            metadata: block_plugin_api::BlockMetadata {
                defined: wire_entries(&self.metadata.defined),
                derived: wire_entries(&self.metadata.derived),
            },
            locked: self.locked,
            deleted: self.deleted,
            sets_access: self.sets_access,
        }
    }

    pub fn decode(info: block_plugin_api::BlockInfo) -> Self {
        Self {
            id: Uuid::from_bytes(info.block_id),
            block_type: Uuid::from_bytes(info.block_type),
            author: info.author.map(Uuid::from_bytes),
            parent: info.parent.map(BlockParent::decode),
            references: info.references.into_iter().map(Uuid::from_bytes).collect(),
            access: info.access,
            metadata: BlockMetadata {
                defined: entries_of(info.metadata.defined),
                derived: entries_of(info.metadata.derived),
            },
            locked: info.locked,
            deleted: info.deleted,
            sets_access: info.sets_access,
        }
    }
}

pub fn wire_entries(entries: &Entries) -> block_plugin_api::MetadataEntries {
    entries
        .iter()
        .map(|(key, value)| (key.into_bytes(), value.clone()))
        .collect()
}

pub fn entries_of(entries: block_plugin_api::MetadataEntries) -> Entries {
    entries
        .into_iter()
        .map(|(key, value)| (Uuid::from_bytes(key), value))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphCommand {
    Create {
        id: Uuid,
        block_type: Uuid,
        parent: BlockParent,
        defined: Entries,
        content: Option<Vec<u8>>,
    },
    SetParent {
        id: Uuid,
        parent: BlockParent,
    },
    Define {
        id: Uuid,
        key: Uuid,
        value: Option<Vec<u8>>,
    },
}

pub(crate) struct GraphState {
    watched: RefCell<BTreeMap<BlockQuery, usize>>,
    watch_changed: Cell<bool>,
    results: RefCell<HashMap<BlockQuery, Ready<Vec<BlockInfo>>>>,
    retired: RefCell<VecDeque<(BlockQuery, Vec<BlockInfo>)>>,
    known: RefCell<HashMap<Uuid, BlockInfo>>,
    revision: OwnedSignal<u64>,
    dirty: Cell<bool>,
    deferred: Cell<u64>,
    ready: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    commands: RefCell<Vec<GraphCommand>>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            watched: RefCell::default(),
            watch_changed: Cell::default(),
            results: RefCell::default(),
            retired: RefCell::default(),
            known: RefCell::default(),
            revision: OwnedSignal::new(0),
            dirty: Cell::default(),
            deferred: Cell::default(),
            ready: RefCell::default(),
            commands: RefCell::default(),
        }
    }
}

impl GraphState {
    pub(crate) fn defer_unless(&self, ready: Rc<dyn Fn() -> bool>) {
        *self.ready.borrow_mut() = Some(ready);
    }

    fn changed(&self) {
        let ready = self.ready.borrow().clone();
        match ready.is_none_or(|ready| ready()) {
            true => self.revision.update(|revision| *revision += 1),
            false => {
                self.dirty.set(true);
                self.deferred.set(self.deferred.get() + 1);
            }
        }
    }

    pub(crate) fn deferred(&self) -> u64 {
        self.deferred.get()
    }

    pub(crate) fn flush(&self) {
        if self.dirty.replace(false) {
            self.revision.update(|revision| *revision += 1);
        }
    }

    pub(crate) fn watch(&self, query: BlockQuery) {
        let mut watched = self.watched.borrow_mut();
        let count = watched.entry(query).or_default();
        *count += 1;
        if *count == 1 {
            self.watch_changed.set(true);
            let mut retired = self.retired.borrow_mut();
            if let Some(at) = retired.iter().position(|(held, _)| *held == query)
                && let Some((_, blocks)) = retired.remove(at)
            {
                self.results.borrow_mut().insert(query, Ok(blocks));
            }
        }
    }

    pub(crate) fn unwatch(&self, query: BlockQuery) {
        let mut watched = self.watched.borrow_mut();
        let Some(count) = watched.get_mut(&query) else {
            return;
        };
        *count -= 1;
        if *count == 0 {
            watched.remove(&query);
            if let Some(Ok(blocks)) = self.results.borrow_mut().remove(&query) {
                let mut retired = self.retired.borrow_mut();
                retired.push_back((query, blocks));
                if retired.len() > RETIRED {
                    retired.pop_front();
                }
            }
            self.watch_changed.set(true);
        }
    }

    pub(crate) fn take_watch(&self) -> Option<Vec<BlockQuery>> {
        self.watch_changed
            .replace(false)
            .then(|| self.watched.borrow().keys().copied().collect())
    }

    pub(crate) fn watched(&self) -> Vec<BlockQuery> {
        self.watched.borrow().keys().copied().collect()
    }

    pub(crate) fn set_result(&self, query: BlockQuery, blocks: Vec<BlockInfo>) {
        {
            let mut known = self.known.borrow_mut();
            for block in &blocks {
                known.insert(block.id, block.clone());
            }
            if let BlockQuery::Block(id) = query
                && blocks.is_empty()
            {
                known.remove(&id);
            }
        }
        self.results.borrow_mut().insert(query, Ok(blocks));
        self.changed();
    }

    pub(crate) fn set_failure(&self, query: BlockQuery, error: String) {
        self.results
            .borrow_mut()
            .insert(query, Err(Pending::Failed(error)));
        self.changed();
    }

    pub(crate) fn result(&self, query: BlockQuery) -> Ready<Vec<BlockInfo>> {
        self.revision.get();
        self.results
            .borrow()
            .get(&query)
            .cloned()
            .unwrap_or(Err(Pending::Loading))
    }

    pub(crate) fn known(&self, id: Uuid) -> Option<BlockInfo> {
        self.revision.get();
        self.known.borrow().get(&id).cloned()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.get()
    }

    pub(crate) fn push(&self, command: GraphCommand) {
        if let GraphCommand::Create {
            id,
            block_type,
            parent,
            defined,
            ..
        } = &command
        {
            let mut info = BlockInfo::new(*id, *block_type, *parent);
            info.metadata.defined.clone_from(defined);
            self.known.borrow_mut().insert(*id, info);
            self.changed();
        }
        self.commands.borrow_mut().push(command);
    }

    pub(crate) fn take_commands(&self) -> Vec<GraphCommand> {
        std::mem::take(&mut self.commands.borrow_mut())
    }
}

#[derive(Clone)]
pub struct Blocks {
    pub(crate) graph: Rc<GraphState>,
    pub(crate) waker: Waker,
    pub(crate) account: Rc<Cell<Uuid>>,
}

impl Blocks {
    pub fn account_id(&self) -> Uuid {
        self.account.get()
    }

    pub fn create<C: be_block::BlockContent>(&self, content: &C, parent: BlockParent) -> Uuid {
        self.create_with(
            C::CONTENT_TYPE,
            Some(content.encode()),
            parent,
            BlockMetadata::default(),
        )
    }

    pub fn create_named<C: be_block::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: impl Into<String>,
    ) -> Uuid {
        self.create_with(
            C::CONTENT_TYPE,
            Some(content.encode()),
            parent,
            BlockMetadata::named(name),
        )
    }

    pub fn create_artifact<C: be_block::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: Option<String>,
        artifact: be_block::Artifact,
    ) -> Uuid {
        let mut metadata = BlockMetadata::default();
        metadata.put(&be_block::NAME, name.as_ref());
        metadata.insert(&be_block::ARTIFACT, &artifact);
        self.create_with(C::CONTENT_TYPE, Some(content.encode()), parent, metadata)
    }

    pub fn create_with(
        &self,
        block_type: Uuid,
        content: Option<Vec<u8>>,
        parent: BlockParent,
        metadata: BlockMetadata,
    ) -> Uuid {
        self.create_at(Uuid::now_v7(), block_type, content, parent, metadata)
    }

    pub fn create_at(
        &self,
        id: Uuid,
        block_type: Uuid,
        content: Option<Vec<u8>>,
        parent: BlockParent,
        metadata: BlockMetadata,
    ) -> Uuid {
        self.graph.push(GraphCommand::Create {
            id,
            block_type,
            parent,
            defined: metadata.defined,
            content,
        });
        self.waker.wake();
        id
    }

    pub fn set_parent(&self, id: Uuid, parent: BlockParent) {
        self.graph.push(GraphCommand::SetParent { id, parent });
        self.waker.wake();
    }

    pub fn set_name(&self, id: Uuid, name: Option<String>) {
        self.define(id, be_block::NAME.definition(name.as_ref()));
    }

    pub fn define(&self, id: Uuid, (key, value): Definition) {
        self.graph.push(GraphCommand::Define { id, key, value });
        self.waker.wake();
    }

    pub fn watch(&self, query: BlockQuery) -> BlockList {
        self.graph.watch(query);
        self.waker.wake();
        let alive = Rc::new(());
        let graph = Rc::downgrade(&self.graph);
        wait_while(Rc::downgrade(&alive), move || match graph.upgrade() {
            Some(graph) => graph.result(query).map(drop),
            None => Ok(()),
        });
        BlockList {
            graph: Rc::clone(&self.graph),
            query,
            _alive: alive,
        }
    }

    pub fn info(&self, id: Uuid) -> Option<BlockInfo> {
        self.graph.known(id)
    }

    pub fn access(&self, id: Uuid) -> AccessLevel {
        self.info(id).map_or(AccessLevel::None, |info| info.access)
    }

    pub fn revision(&self) -> u64 {
        self.graph.revision()
    }
}

pub struct BlockList {
    graph: Rc<GraphState>,
    query: BlockQuery,
    _alive: Rc<()>,
}

impl BlockList {
    pub fn read(&self) -> Ready<Vec<BlockInfo>> {
        self.graph.result(self.query)
    }

    pub fn query(&self) -> BlockQuery {
        self.query
    }
}

impl Drop for BlockList {
    fn drop(&mut self) {
        self.graph.unwatch(self.query);
    }
}
