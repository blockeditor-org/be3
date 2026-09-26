use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use block_editor_beui::be_block::{BlockContent, LiveEdit};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, SeededContent};
use block_plugin_api::{ContentOperation, EditorInstanceId, EditorMessage, Message};
use uuid::Uuid;

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
    Written {
        content_type: Uuid,
        bytes: Vec<u8>,
        unapplied: Vec<Vec<u8>>,
    },
}

impl Stored {
    fn written(content_type: Uuid, bytes: Vec<u8>) -> Self {
        Self::Written {
            content_type,
            bytes,
            unapplied: Vec::new(),
        }
    }

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

    fn apply(&mut self, operation: &[u8]) {
        match self {
            Self::Typed(held) => held.apply(operation),
            Self::Written { unapplied, .. } => unapplied.push(operation.to_vec()),
        }
    }

    fn typed<C: LiveEdit>(&mut self) -> &C {
        if let Self::Written {
            bytes, unapplied, ..
        } = self
        {
            let mut content = C::decode(bytes).expect("the block holds that content type");
            for operation in unapplied.iter() {
                Held::apply(&mut content, operation);
            }
            *self = Self::Typed(Box::new(content));
        }
        let Self::Typed(held) = self else {
            unreachable!("the content was just typed")
        };
        held.as_any()
            .downcast_ref::<C>()
            .expect("the store holds that block with that content type")
    }
}

struct Block {
    content: Stored,
    revision: u64,
    base: u64,
    log: Vec<(Vec<u8>, bool)>,
    taken: u64,
    sent: Option<u64>,
}

impl Block {
    fn new(content: Stored) -> Self {
        Self {
            content,
            revision: 1,
            base: 1,
            log: Vec::new(),
            taken: 0,
            sent: None,
        }
    }

    fn replace(&mut self, content: Stored) {
        self.content = content;
        self.revision += 1;
        self.base = self.revision;
        self.log.clear();
    }

    fn operate(&mut self, operation: Vec<u8>, mine: bool) {
        self.content.apply(&operation);
        self.revision += 1;
        self.log.push((operation, mine));
        if mine {
            self.taken += 1;
        }
    }

    fn update(&mut self, instance: EditorInstanceId, block: Uuid) -> Option<Message> {
        if self.sent == Some(self.revision) {
            return None;
        }
        let message = match self.sent {
            Some(sent) if sent >= self.base => EditorMessage::ContentOperations {
                instance,
                block_id: block.into_bytes(),
                operations: self.log[(sent - self.base) as usize..]
                    .iter()
                    .map(|(operation, mine)| ContentOperation {
                        operation: operation.clone(),
                        mine: *mine,
                    })
                    .collect(),
            },
            _ => EditorMessage::Content {
                instance,
                block_id: block.into_bytes(),
                content_type: self.content.content_type().into_bytes(),
                bytes: self.content.bytes(),
                applied: self.taken,
            },
        };
        self.sent = Some(self.revision);
        Some(Message::Editor(message))
    }
}

#[derive(Default)]
struct Inner {
    own: Option<Uuid>,
    workspace: Uuid,
    blocks: BTreeMap<Uuid, Block>,
    seeded: Vec<SeededContent>,
    graph: BTreeMap<Uuid, BlockInfo>,
    created: Vec<Uuid>,
    watched_content: BTreeSet<Uuid>,
    watched_queries: Vec<BlockQuery>,
    answered: BTreeMap<BlockQuery, Vec<BlockInfo>>,
    deferred: Option<Vec<(Uuid, Vec<u8>)>>,
}

#[derive(Clone, Default)]
pub struct ContentStore(Rc<RefCell<Inner>>);

impl ContentStore {
    pub(crate) fn new(workspace: Uuid) -> Self {
        let store = Self::default();
        store.0.borrow_mut().workspace = workspace;
        store
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
    }

    pub fn block(&self, id: Uuid) -> Option<BlockInfo> {
        self.0.borrow().graph.get(&id).cloned()
    }

    pub fn created(&self) -> Vec<Uuid> {
        self.0.borrow().created.clone()
    }

    pub fn watches(&self, block: Uuid) -> bool {
        self.0.borrow().watched_content.contains(&block)
    }

    fn key(&self, block: Option<Uuid>) -> Uuid {
        block
            .or(self.0.borrow().own)
            .expect("this editor has no block of its own; name the block")
    }

    pub fn hold<C: LiveEdit>(&self, block: Option<Uuid>, content: C) {
        let key = self.key(block);
        let content = Stored::Typed(Box::new(content));
        let mut inner = self.0.borrow_mut();
        match inner.blocks.get_mut(&key) {
            Some(held) => held.replace(content),
            None => {
                inner.blocks.insert(key, Block::new(content));
            }
        }
    }

    pub fn content<C: LiveEdit + Clone>(&self, block: Option<Uuid>) -> C {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let held = inner
            .blocks
            .get_mut(&key)
            .expect("the store holds that block");
        held.content.typed::<C>().clone()
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

    pub fn edit<C: LiveEdit>(&self, block: Option<Uuid>, operation: &C::Op) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let held = inner
            .blocks
            .get_mut(&key)
            .expect("the store holds that block");
        held.content.typed::<C>();
        held.operate(C::encode_operation(operation), false);
    }

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
        for (block, operation) in deferred {
            self.receive(&EditorMessage::Operate {
                instance: EditorInstanceId(0),
                block_id: block.into_bytes(),
                operation,
            });
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
        let content = Stored::written(content_type, bytes);
        match inner.blocks.get_mut(&block) {
            Some(held) if replace => held.replace(content),
            Some(_) => {}
            None => {
                inner.blocks.insert(block, Block::new(content));
            }
        }
    }

    pub(crate) fn receive(&self, message: &EditorMessage) {
        match message {
            EditorMessage::Operate {
                block_id,
                operation,
                ..
            } => {
                let block = Uuid::from_bytes(*block_id);
                let mut inner = self.0.borrow_mut();
                if let Some(deferred) = &mut inner.deferred {
                    deferred.push((block, operation.clone()));
                    return;
                }
                let linked = inner.own == Some(block) || inner.watched_content.contains(&block);
                if let (true, Some(held)) = (linked, inner.blocks.get_mut(&block)) {
                    held.operate(operation.clone(), true);
                }
            }
            EditorMessage::WatchContent { blocks, .. } => {
                self.0.borrow_mut().watched_content = blocks
                    .iter()
                    .map(|watched| Uuid::from_bytes(watched.block_id))
                    .collect();
            }
            EditorMessage::SeedContent {
                block_id,
                content_type,
                bytes,
                ..
            } => self.seed(
                Uuid::from_bytes(*block_id),
                Uuid::from_bytes(*content_type),
                bytes.clone(),
                false,
            ),
            EditorMessage::ReplaceContent {
                block_id,
                content_type,
                bytes,
                ..
            } => self.seed(
                Uuid::from_bytes(*block_id),
                Uuid::from_bytes(*content_type),
                bytes.clone(),
                true,
            ),
            EditorMessage::CreateBlock {
                block_id,
                content_type,
                parent,
                name,
                artifact,
                content,
                ..
            } => {
                let id = Uuid::from_bytes(*block_id);
                let block_type = Uuid::from_bytes(*content_type);
                let info = BlockInfo::decode(block_plugin_api::BlockInfo {
                    block_id: *block_id,
                    block_type: *content_type,
                    author: Uuid::nil().into_bytes(),
                    parent: *parent,
                    name: name.clone(),
                    named_by_hand: name.is_some(),
                    references: Vec::new(),
                    access: block_plugin_api::AccessLevel::Edit,
                    artifact: artifact.clone(),
                });
                {
                    let mut inner = self.0.borrow_mut();
                    inner.graph.insert(id, info);
                    inner.created.push(id);
                }
                if let Some(bytes) = content {
                    self.seed(id, block_type, bytes.to_vec(), false);
                }
            }
            EditorMessage::SetParent {
                block_id, parent, ..
            } => {
                if let Some(info) = self
                    .0
                    .borrow_mut()
                    .graph
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    info.parent = BlockParent::decode(*parent);
                }
            }
            EditorMessage::SetName { block_id, name, .. } => {
                if let Some(info) = self
                    .0
                    .borrow_mut()
                    .graph
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    info.named_by_hand = name.is_some();
                    if name.is_some() {
                        info.name.clone_from(name);
                    }
                }
            }
            EditorMessage::WatchBlocks { queries, .. } => {
                self.0.borrow_mut().watched_queries =
                    queries.iter().copied().map(BlockQuery::decode).collect();
            }
            _ => {}
        }
    }

    pub(crate) fn outgoing(&self, instance: EditorInstanceId) -> Vec<Message> {
        let mut messages = Vec::new();
        {
            let mut inner = self.0.borrow_mut();
            let linked: Vec<Uuid> = inner
                .own
                .into_iter()
                .chain(inner.watched_content.iter().copied())
                .collect();
            for block in linked {
                if let Some(held) = inner.blocks.get_mut(&block) {
                    messages.extend(held.update(instance, block));
                }
            }
        }
        let queries = self.0.borrow().watched_queries.clone();
        for query in queries {
            let listed = self.query(query);
            let mut inner = self.0.borrow_mut();
            if inner.answered.get(&query) == Some(&listed) {
                continue;
            }
            inner.answered.insert(query, listed.clone());
            messages.push(Message::Editor(EditorMessage::Blocks {
                instance,
                query: query.encode(),
                blocks: listed.iter().map(BlockInfo::encode).collect(),
            }));
        }
        messages
    }

    pub(crate) fn pending(&self) -> bool {
        let owed = {
            let inner = self.0.borrow();
            inner
                .own
                .iter()
                .chain(inner.watched_content.iter())
                .filter_map(|block| inner.blocks.get(block))
                .any(|held| held.sent != Some(held.revision))
        };
        owed || {
            let queries = self.0.borrow().watched_queries.clone();
            queries.into_iter().any(|query| {
                let listed = self.query(query);
                self.0.borrow().answered.get(&query) != Some(&listed)
            })
        }
    }

    fn references_of(&self, id: Uuid) -> Vec<Uuid> {
        let inner = self.0.borrow();
        match inner.blocks.get(&id).map(|held| &held.content) {
            Some(Stored::Typed(content)) => content.references(inner.workspace),
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
}
