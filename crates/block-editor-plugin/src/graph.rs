use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use be_block::ArtifactSource;
use block_plugin_api::{AccessLevel, BlockLocation};
use uuid::Uuid;

use crate::host::Waker;

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
    Children(Uuid),
    References(Uuid),
    Backrefs(Uuid),
    Parents(Uuid),
    Block(Uuid),
}

impl BlockQuery {
    pub fn encode(self) -> block_plugin_api::BlockQuery {
        use block_plugin_api::BlockQuery as Wire;
        match self {
            Self::Roots => Wire::Roots,
            Self::Detached => Wire::Detached,
            Self::Children(id) => Wire::Children(id.into_bytes()),
            Self::References(id) => Wire::References(id.into_bytes()),
            Self::Backrefs(id) => Wire::Backrefs(id.into_bytes()),
            Self::Parents(id) => Wire::Parents(id.into_bytes()),
            Self::Block(id) => Wire::Block(id.into_bytes()),
        }
    }

    pub fn decode(query: block_plugin_api::BlockQuery) -> Self {
        use block_plugin_api::BlockQuery as Wire;
        match query {
            Wire::Roots => Self::Roots,
            Wire::Detached => Self::Detached,
            Wire::Children(id) => Self::Children(Uuid::from_bytes(id)),
            Wire::References(id) => Self::References(Uuid::from_bytes(id)),
            Wire::Backrefs(id) => Self::Backrefs(Uuid::from_bytes(id)),
            Wire::Parents(id) => Self::Parents(Uuid::from_bytes(id)),
            Wire::Block(id) => Self::Block(Uuid::from_bytes(id)),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockInfo {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Uuid,
    pub parent: BlockParent,
    pub name: Option<String>,
    pub named_by_hand: bool,
    pub references: Vec<Uuid>,
    pub access: AccessLevel,
    pub artifact: Option<ArtifactSource>,
}

impl BlockInfo {
    pub fn new(id: Uuid, block_type: Uuid, parent: BlockParent) -> Self {
        Self {
            id,
            block_type,
            author: Uuid::nil(),
            parent,
            name: None,
            named_by_hand: false,
            references: Vec::new(),
            access: AccessLevel::Edit,
            artifact: None,
        }
    }

    pub fn is_artifact(&self) -> bool {
        self.artifact.is_some()
    }

    pub fn label(&self, types: &dyn block_ui::BlockTypes) -> block_ui::BlockLabel {
        block_ui::BlockLabel::new(
            types,
            self.block_type,
            self.name.as_deref(),
            self.named_by_hand,
        )
    }

    pub fn encode(&self) -> block_plugin_api::BlockInfo {
        block_plugin_api::BlockInfo {
            block_id: self.id.into_bytes(),
            block_type: self.block_type.into_bytes(),
            author: self.author.into_bytes(),
            parent: self.parent.encode(),
            name: self.name.clone(),
            named_by_hand: self.named_by_hand,
            references: self.references.iter().map(|id| id.into_bytes()).collect(),
            access: self.access,
            artifact: self
                .artifact
                .as_ref()
                .map(|artifact| block_plugin_api::ArtifactSource {
                    source_type: artifact.source_type.into_bytes(),
                    data: artifact.data.clone(),
                }),
        }
    }

    pub fn decode(info: block_plugin_api::BlockInfo) -> Self {
        Self {
            id: Uuid::from_bytes(info.block_id),
            block_type: Uuid::from_bytes(info.block_type),
            author: Uuid::from_bytes(info.author),
            parent: BlockParent::decode(info.parent),
            name: info.name,
            named_by_hand: info.named_by_hand,
            references: info.references.into_iter().map(Uuid::from_bytes).collect(),
            access: info.access,
            artifact: info.artifact.map(|artifact| ArtifactSource {
                source_type: Uuid::from_bytes(artifact.source_type),
                data: artifact.data,
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphCommand {
    Create {
        id: Uuid,
        block_type: Uuid,
        parent: BlockParent,
        name: Option<String>,
        artifact: Option<ArtifactSource>,
        content: Option<Vec<u8>>,
    },
    SetParent {
        id: Uuid,
        parent: BlockParent,
    },
    SetName {
        id: Uuid,
        name: Option<String>,
    },
}

#[derive(Default)]
pub(crate) struct GraphState {
    watched: RefCell<BTreeMap<BlockQuery, usize>>,
    watch_changed: Cell<bool>,
    results: RefCell<HashMap<BlockQuery, Vec<BlockInfo>>>,
    known: RefCell<HashMap<Uuid, BlockInfo>>,
    revision: Cell<u64>,
    commands: RefCell<Vec<GraphCommand>>,
}

impl GraphState {
    pub(crate) fn watch(&self, query: BlockQuery) {
        let mut watched = self.watched.borrow_mut();
        let count = watched.entry(query).or_default();
        *count += 1;
        if *count == 1 {
            self.watch_changed.set(true);
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
            self.results.borrow_mut().remove(&query);
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
        self.results.borrow_mut().insert(query, blocks);
        self.revision.set(self.revision.get() + 1);
    }

    pub(crate) fn result(&self, query: BlockQuery) -> Option<Vec<BlockInfo>> {
        self.results.borrow().get(&query).cloned()
    }

    pub(crate) fn known(&self, id: Uuid) -> Option<BlockInfo> {
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
            name,
            artifact,
            ..
        } = &command
        {
            let mut info = BlockInfo::new(*id, *block_type, *parent);
            info.name.clone_from(name);
            info.named_by_hand = name.is_some();
            info.artifact.clone_from(artifact);
            self.known.borrow_mut().insert(*id, info);
            self.revision.set(self.revision.get() + 1);
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
        self.create_with(C::CONTENT_TYPE, Some(content.encode()), parent, None, None)
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
            Some(name.into()),
            None,
        )
    }

    pub fn create_artifact<C: be_block::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: Option<String>,
        artifact: ArtifactSource,
    ) -> Uuid {
        self.create_with(
            C::CONTENT_TYPE,
            Some(content.encode()),
            parent,
            name,
            Some(artifact),
        )
    }

    pub fn create_with(
        &self,
        block_type: Uuid,
        content: Option<Vec<u8>>,
        parent: BlockParent,
        name: Option<String>,
        artifact: Option<ArtifactSource>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        self.graph.push(GraphCommand::Create {
            id,
            block_type,
            parent,
            name,
            artifact,
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
        self.graph.push(GraphCommand::SetName { id, name });
        self.waker.wake();
    }

    pub fn watch(&self, query: BlockQuery) -> BlockList {
        self.graph.watch(query);
        self.waker.wake();
        BlockList {
            graph: Rc::clone(&self.graph),
            query,
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
}

impl BlockList {
    pub fn read(&self) -> Vec<BlockInfo> {
        self.graph.result(self.query).unwrap_or_default()
    }

    pub fn is_loaded(&self) -> bool {
        self.graph.result(self.query).is_some()
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
