use be_lib_stable_hash::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

use be_block_content::{BlockMetadata, Definition};
use be_sdk::{
    AccessLevel, BlockGraph, BlockInfo, BlockParent, BlockQuery, BlockWatch, EditorHost,
    Subscription,
};
use beui::reactive::*;
use uuid::Uuid;

use crate::content::ready;

#[derive(Default)]
struct Known {
    changes: RefCell<HashMap<Uuid, (Subscription, OwnedTrigger)>>,
}

#[derive(Clone)]
pub struct Blocks {
    graph: BlockGraph,
    known: Rc<Known>,
}

impl Blocks {
    pub fn of(host: &EditorHost) -> Self {
        Self {
            graph: host.graph(),
            known: host.extension::<Known>(),
        }
    }

    pub fn graph(&self) -> &BlockGraph {
        &self.graph
    }

    pub fn account_id(&self) -> Uuid {
        self.graph.account_id()
    }

    pub fn create<C: be_block_content::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
    ) -> Uuid {
        self.graph.create(content, parent)
    }

    pub fn create_named<C: be_block_content::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: impl Into<String>,
    ) -> Uuid {
        self.graph.create_named(content, parent, name)
    }

    pub fn create_with(
        &self,
        block_type: Uuid,
        content: Option<Vec<u8>>,
        parent: BlockParent,
        metadata: BlockMetadata,
    ) -> Uuid {
        self.graph
            .create_with(block_type, content, parent, metadata)
    }

    pub fn create_at(
        &self,
        id: Uuid,
        block_type: Uuid,
        content: Option<Vec<u8>>,
        parent: BlockParent,
        metadata: BlockMetadata,
        owner_only: bool,
    ) -> Uuid {
        self.graph
            .create_at(id, block_type, content, parent, metadata, owner_only)
    }

    pub fn set_parent(&self, id: Uuid, parent: BlockParent) {
        self.graph.set_parent(id, parent);
    }

    /// Names the block, or clears its name; refuses a name past
    /// `be_block_content::metadata::MAX_NAME_BYTES` (a rename shows why), where one taken from
    /// content is cut to it first (`metadata::clip_name`).
    pub fn set_name(&self, id: Uuid, name: Option<String>) -> Result<(), String> {
        use be_block_content::metadata::{MAX_NAME_BYTES, name_too_long};
        if name
            .as_ref()
            .is_some_and(|name| name.len() > MAX_NAME_BYTES)
        {
            return Err(name_too_long());
        }
        self.graph.set_name(id, name);
        Ok(())
    }

    pub fn define(&self, id: Uuid, definition: Definition) {
        self.graph.define(id, definition);
    }

    /// The blocks `query` finds, loading until the graph answers it.
    pub fn watch(&self, query: BlockQuery) -> BlockList {
        let watch = Rc::new(self.graph.watch(query));
        let changed = OwnedTrigger::new();
        let answered = OwnedSource::from(ready(watch.read()).map(drop));
        let (change, answer) = (changed.handle(), answered.handle());
        let held = Rc::downgrade(&watch);
        let changes = watch.on_change(move || {
            if let Some(watch) = held.upgrade() {
                answer.set(ready(watch.read()).map(drop));
            }
            change.notify();
        });
        BlockList {
            _changes: changes,
            changed,
            answered,
            watch,
        }
    }

    pub fn info(&self, id: Uuid) -> Option<BlockInfo> {
        self.track(id);
        self.graph.info(id)
    }

    pub fn access(&self, id: Uuid) -> AccessLevel {
        self.info(id).map_or(AccessLevel::None, |info| info.access)
    }

    fn track(&self, id: Uuid) {
        let changed = self
            .known
            .changes
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| {
                let changed = OwnedTrigger::new();
                let change = changed.handle();
                let changes = self.graph.on_info(id, move || change.notify());
                (changes, changed)
            })
            .1
            .handle();
        changed.track();
    }
}

pub struct BlockList {
    _changes: Subscription,
    changed: OwnedTrigger,
    /// Whether the graph answered: what a read waits on.
    answered: OwnedSource<()>,
    watch: Rc<BlockWatch>,
}

impl BlockList {
    /// The blocks found, waiting until the graph answers (at the nearest `Loading`, when read
    /// outside a memo; a memo that reads it is pending until then).
    pub fn read(&self) -> Ready<Vec<BlockInfo>> {
        self.changed.track();
        self.answered.get()?;
        ready(self.watch.read())
    }

    pub fn query(&self) -> BlockQuery {
        self.watch.query()
    }
}
