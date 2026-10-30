use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use be_block::{BlockMetadata, Definition};
use beui::reactive::*;
use block_editor_plugin::{
    AccessLevel, BlockGraph, BlockInfo, BlockParent, BlockQuery, BlockWatch, EditorHost,
    Subscription,
};
use uuid::Uuid;

use crate::content::ready;

#[derive(Default)]
struct Known {
    changes: RefCell<HashMap<Uuid, (Subscription, OwnedSignal<u64>)>>,
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

    pub fn create<C: be_block::BlockContent>(&self, content: &C, parent: BlockParent) -> Uuid {
        self.graph.create(content, parent)
    }

    pub fn create_named<C: be_block::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: impl Into<String>,
    ) -> Uuid {
        self.graph.create_named(content, parent, name)
    }

    pub fn create_artifact<C: be_block::BlockContent>(
        &self,
        content: &C,
        parent: BlockParent,
        name: Option<String>,
        artifact: be_block::Artifact,
    ) -> Uuid {
        self.graph.create_artifact(content, parent, name, artifact)
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
    ) -> Uuid {
        self.graph
            .create_at(id, block_type, content, parent, metadata)
    }

    pub fn set_parent(&self, id: Uuid, parent: BlockParent) {
        self.graph.set_parent(id, parent);
    }

    pub fn set_name(&self, id: Uuid, name: Option<String>) {
        self.graph.set_name(id, name);
    }

    pub fn define(&self, id: Uuid, definition: Definition) {
        self.graph.define(id, definition);
    }

    pub fn watch(&self, query: BlockQuery) -> BlockList {
        let watch = Rc::new(self.graph.watch(query));
        let changed = OwnedSignal::new(0_u64);
        let bumped = changed.handle();
        let changes = watch.on_change(move || bumped.update(|count| *count += 1));
        let held = Rc::downgrade(&watch);
        wait_while(Rc::downgrade(&watch), move || match held.upgrade() {
            Some(watch) => {
                bumped.get();
                ready(watch.read()).map(drop)
            }
            None => Ok(()),
        });
        BlockList {
            _changes: changes,
            changed,
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
                let changed = OwnedSignal::new(0_u64);
                let bumped = changed.handle();
                let changes = self
                    .graph
                    .on_info(id, move || bumped.update(|count| *count += 1));
                (changes, changed)
            })
            .1
            .handle();
        changed.get();
    }
}

pub struct BlockList {
    _changes: Subscription,
    changed: OwnedSignal<u64>,
    watch: Rc<BlockWatch>,
}

impl BlockList {
    pub fn read(&self) -> Ready<Vec<BlockInfo>> {
        self.changed.get();
        ready(self.watch.read())
    }

    pub fn query(&self) -> BlockQuery {
        self.watch.query()
    }
}
