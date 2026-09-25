use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use block_editor_beui::be_block::{BlockContent, LiveEdit};
use block_editor_beui::{
    BeuiApp, BlockInfo, BlockParent, BlockQuery, EditorHost, GraphCommand, SeededContent,
};
use uuid::Uuid;

use crate::BeuiTest;

trait Held: Any {
    fn apply(&mut self, operation: &[u8]);

    fn encode(&self) -> Vec<u8>;

    fn content_type(&self) -> Uuid;

    fn references(&self, workspace: Uuid) -> Vec<Uuid>;

    fn as_any(&self) -> &dyn Any;
}

impl<C: LiveEdit> Held for C {
    fn apply(&mut self, operation: &[u8]) {
        let operation = C::decode_operation(operation)
            .expect("the editor sent an operation its content type cannot read");
        LiveEdit::apply(self, &operation);
    }

    fn encode(&self) -> Vec<u8> {
        BlockContent::encode(self)
    }

    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn references(&self, workspace: Uuid) -> Vec<Uuid> {
        BlockContent::references_in(self, workspace)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

enum Stored {
    Typed(Box<dyn Held>),
    Written { content_type: Uuid, bytes: Vec<u8> },
}

impl Stored {
    fn content_type(&self) -> Uuid {
        match self {
            Self::Typed(held) => held.content_type(),
            Self::Written { content_type, .. } => *content_type,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Typed(held) => held.encode(),
            Self::Written { bytes, .. } => bytes.clone(),
        }
    }
}

struct Block {
    content: Stored,
    applied: u64,
}

struct Inner {
    host: EditorHost,
    own: Option<Uuid>,
    blocks: BTreeMap<Option<Uuid>, Block>,
    seeded: Vec<SeededContent>,
    graph: BTreeMap<Uuid, BlockInfo>,
    answered: BTreeMap<BlockQuery, Vec<BlockInfo>>,
    created: Vec<Uuid>,
}

#[derive(Clone)]
pub struct ContentStore(Rc<RefCell<Inner>>);

impl ContentStore {
    pub fn new(host: EditorHost) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            host,
            own: None,
            blocks: BTreeMap::new(),
            seeded: Vec::new(),
            graph: BTreeMap::new(),
            answered: BTreeMap::new(),
            created: Vec::new(),
        })))
    }

    pub fn own(&self, block: Uuid, block_type: Uuid) {
        let mut inner = self.0.borrow_mut();
        inner.own = Some(block);
        inner
            .graph
            .entry(block)
            .or_insert_with(|| BlockInfo::new(block, block_type, BlockParent::Root));
    }

    pub fn add_block(&self, info: BlockInfo) {
        self.0.borrow_mut().graph.insert(info.id, info);
        self.answer();
    }

    pub fn block(&self, id: Uuid) -> Option<BlockInfo> {
        self.sync();
        self.0.borrow().graph.get(&id).cloned()
    }

    pub fn created(&self) -> Vec<Uuid> {
        self.sync();
        self.0.borrow().created.clone()
    }

    fn apply_graph(&self) -> bool {
        let commands = self.0.borrow().host.take_graph_commands();
        let changed = !commands.is_empty();
        for command in commands {
            let mut inner = self.0.borrow_mut();
            match command {
                GraphCommand::Create {
                    id,
                    block_type,
                    parent,
                    name,
                    artifact,
                    content,
                } => {
                    let mut info = BlockInfo::new(id, block_type, parent);
                    info.named_by_hand = name.is_some();
                    info.name = name;
                    info.artifact = artifact;
                    inner.graph.insert(id, info);
                    inner.created.push(id);
                    if let Some(bytes) = content {
                        inner.seeded.push(SeededContent {
                            block: id,
                            content_type: block_type,
                            bytes: bytes.clone(),
                            replace: false,
                        });
                        inner.blocks.insert(
                            Some(id),
                            Block {
                                content: Stored::Written {
                                    content_type: block_type,
                                    bytes,
                                },
                                applied: 0,
                            },
                        );
                    }
                }
                GraphCommand::SetParent { id, parent } => {
                    if let Some(info) = inner.graph.get_mut(&id) {
                        info.parent = parent;
                    }
                }
                GraphCommand::SetName { id, name } => {
                    if let Some(info) = inner.graph.get_mut(&id) {
                        info.named_by_hand = name.is_some();
                        if name.is_some() {
                            info.name = name;
                        }
                    }
                }
            }
        }
        changed
    }

    fn references_of(&self, id: Uuid) -> Vec<Uuid> {
        let inner = self.0.borrow();
        let key = match inner.own == Some(id) {
            true => None,
            false => Some(id),
        };
        let workspace = inner.host.workspace_id();
        match inner.blocks.get(&key).map(|held| &held.content) {
            Some(Stored::Typed(content)) => content.references(workspace),
            _ => inner
                .graph
                .get(&id)
                .map(|info| info.references.clone())
                .unwrap_or_default(),
        }
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
                .filter(|node| node.parent == BlockParent::Root)
                .cloned()
                .collect(),
            BlockQuery::Detached => nodes
                .iter()
                .filter(|node| node.parent == BlockParent::Detached)
                .cloned()
                .collect(),
            BlockQuery::Children(parent) => nodes
                .iter()
                .filter(|node| node.parent == BlockParent::Block(parent))
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
                let mut current = known(&block).and_then(|node| node.parent.block());
                while let Some(parent) = current {
                    let Some(node) = known(&parent) else {
                        break;
                    };
                    if chain.iter().any(|seen: &BlockInfo| seen.id == parent) {
                        break;
                    }
                    current = node.parent.block();
                    chain.push(node);
                }
                chain.reverse();
                chain
            }
            BlockQuery::Block(block) => known(&block).into_iter().collect(),
        };
        listed.into_iter().map(with_references).collect()
    }

    fn answer(&self) -> bool {
        let watched = self.0.borrow().host.watched_blocks();
        let mut changed = false;
        for query in watched {
            let listed = self.query(query);
            let mut inner = self.0.borrow_mut();
            if inner.answered.get(&query) == Some(&listed) {
                continue;
            }
            inner.answered.insert(query, listed.clone());
            inner.host.set_blocks(query, listed);
            changed = true;
        }
        changed
    }

    pub fn hold<C: LiveEdit>(&self, block: Option<Uuid>, content: C) {
        self.0.borrow_mut().blocks.insert(
            block,
            Block {
                content: Stored::Typed(Box::new(content)),
                applied: 0,
            },
        );
        self.publish(block);
    }

    pub fn content<C: LiveEdit + Clone>(&self, block: Option<Uuid>) -> C {
        self.sync();
        let inner = self.0.borrow();
        let held = inner
            .blocks
            .get(&block)
            .expect("the store holds that block");
        match &held.content {
            Stored::Typed(content) => content
                .as_any()
                .downcast_ref::<C>()
                .expect("the store holds that block with that content type")
                .clone(),
            Stored::Written { bytes, .. } => {
                C::decode(bytes).expect("the block holds that content type")
            }
        }
    }

    pub fn holds(&self, block: Option<Uuid>) -> bool {
        self.sync();
        self.0.borrow().blocks.contains_key(&block)
    }

    pub fn edit<C: LiveEdit>(&self, block: Option<Uuid>, operation: &C::Op) {
        {
            let mut inner = self.0.borrow_mut();
            let held = inner
                .blocks
                .get_mut(&block)
                .expect("the store holds that block");
            let Stored::Typed(content) = &mut held.content else {
                panic!("an edit needs the block's content type; hold it first");
            };
            content.apply(&C::encode_operation(operation));
        }
        self.publish(block);
    }

    pub fn seeded(&self) -> Vec<SeededContent> {
        self.sync();
        self.0.borrow().seeded.clone()
    }

    pub fn sync(&self) -> bool {
        let graphed = self.apply_graph();
        let written = {
            let inner = self.0.borrow();
            inner.host.take_seeded_content()
        };
        let mut changed = Vec::new();
        for seeded in written {
            let mut inner = self.0.borrow_mut();
            let key = Some(seeded.block);
            let exists = inner.blocks.contains_key(&key);
            if seeded.replace || !exists {
                inner.blocks.insert(
                    key,
                    Block {
                        content: Stored::Written {
                            content_type: seeded.content_type,
                            bytes: seeded.bytes.clone(),
                        },
                        applied: 0,
                    },
                );
                changed.push(key);
            }
            inner.seeded.push(seeded);
        }
        let keys: Vec<Option<Uuid>> = self.0.borrow().blocks.keys().copied().collect();
        for key in keys {
            let mut inner = self.0.borrow_mut();
            let operations = match key {
                Some(block) => inner.host.take_content_operations_of(block),
                None => inner.host.take_content_operations(),
            };
            if operations.is_empty() {
                continue;
            }
            let Some(held) = inner.blocks.get_mut(&key) else {
                continue;
            };
            let Stored::Typed(content) = &mut held.content else {
                continue;
            };
            for operation in &operations {
                content.apply(operation);
                held.applied += 1;
            }
            changed.push(key);
        }
        for key in &changed {
            self.publish(*key);
        }
        let answered = self.answer();
        graphed || answered || !changed.is_empty()
    }

    fn publish(&self, block: Option<Uuid>) {
        let inner = self.0.borrow();
        let Some(held) = inner.blocks.get(&block) else {
            return;
        };
        let (content_type, bytes) = (held.content.content_type(), held.content.bytes());
        match block {
            Some(block) => inner
                .host
                .set_content_of(block, content_type, bytes, held.applied),
            None => inner
                .host
                .set_block_content(content_type, bytes, held.applied),
        }
    }
}

pub struct ContentHarness<A: BeuiApp> {
    pub editor: BeuiTest<A>,
    pub host: EditorHost,
    store: ContentStore,
}

impl<A: BeuiApp> ContentHarness<A> {
    pub fn new(editor: BeuiTest<A>, host: EditorHost) -> Self {
        let store = ContentStore::new(host.clone());
        if let Some(block) = editor.block_id() {
            store.own(block, host.block_type().unwrap_or_default());
        }
        Self {
            editor,
            host,
            store,
        }
    }

    pub fn store(&self) -> ContentStore {
        self.store.clone()
    }

    pub fn hold<C: LiveEdit>(&mut self, block: Option<Uuid>, content: C) {
        self.store.hold(block, content);
    }

    pub fn content<C: LiveEdit + Clone>(&self, block: Option<Uuid>) -> C {
        self.store.content(block)
    }

    pub fn edit<C: LiveEdit>(&mut self, block: Option<Uuid>, operation: &C::Op) {
        self.store.edit::<C>(block, operation);
        self.editor.run();
    }

    pub fn seeded(&mut self) -> Vec<SeededContent> {
        self.store.seeded()
    }

    pub fn run(&mut self) {
        self.editor.run();
        for _ in 0..4 {
            if !self.store.sync() {
                break;
            }
            self.editor.run();
        }
    }
}

impl<A: BeuiApp> std::ops::Deref for ContentHarness<A> {
    type Target = BeuiTest<A>;

    fn deref(&self) -> &BeuiTest<A> {
        &self.editor
    }
}

impl<A: BeuiApp> std::ops::DerefMut for ContentHarness<A> {
    fn deref_mut(&mut self) -> &mut BeuiTest<A> {
        &mut self.editor
    }
}
