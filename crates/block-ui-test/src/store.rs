use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use block_editor_beui::be_block::history::History;
use block_editor_beui::be_block::{BlockContent, LiveEdit, Undo};
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, SeededContent};
use block_plugin_api::{
    BlockCommand, ContentOperation, EditorInstanceId, EditorMessage, GesturePhase, Message,
};
use uuid::Uuid;

trait Held: Any {
    fn apply(&mut self, operation: &[u8]);

    fn encode(&self) -> Vec<u8>;

    fn session_state(&self) -> Vec<u8>;

    fn content_type(&self) -> Uuid;

    fn references(&self, organization: Uuid) -> Vec<Uuid>;

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

    fn session_state(&self) -> Vec<u8> {
        LiveEdit::session_state(self)
    }

    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn references(&self, organization: Uuid) -> Vec<Uuid> {
        BlockContent::references_in(self, organization)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

trait Recorder {
    fn record(&mut self, content: &dyn Any, operation: &[u8]);

    fn begin(&mut self);

    fn commit(&mut self);

    fn cancel(&mut self, content: &dyn Any) -> Vec<Vec<u8>>;

    fn step(&mut self, content: &dyn Any, redo: bool) -> Vec<Vec<u8>>;
}

impl<C: Undo> Recorder for History<C> {
    fn record(&mut self, content: &dyn Any, operation: &[u8]) {
        if let (Some(content), Ok(operation)) =
            (content.downcast_ref::<C>(), C::decode_operation(operation))
        {
            History::record(self, content, &operation, None, None);
        }
    }

    fn begin(&mut self) {
        History::begin(self, None);
    }

    fn commit(&mut self) {
        History::commit(self, None);
    }

    fn cancel(&mut self, content: &dyn Any) -> Vec<Vec<u8>> {
        let Some(content) = content.downcast_ref::<C>() else {
            return Vec::new();
        };
        History::cancel(self, content, None)
            .iter()
            .map(C::encode_operation)
            .collect()
    }

    fn step(&mut self, content: &dyn Any, redo: bool) -> Vec<Vec<u8>> {
        let Some(content) = content.downcast_ref::<C>() else {
            return Vec::new();
        };
        let operations = match redo {
            false => self.undo(content),
            true => self.redo(content),
        };
        operations.iter().map(C::encode_operation).collect()
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

    fn session_state(&self) -> Vec<u8> {
        match self {
            Self::Typed(held) => held.session_state(),
            Self::Written { .. } => Vec::new(),
        }
    }

    fn apply(&mut self, operation: &[u8]) {
        match self {
            Self::Typed(held) => held.apply(operation),
            Self::Written { unapplied, .. } => unapplied.push(operation.to_vec()),
        }
    }

    fn as_any(&self) -> Option<&dyn Any> {
        match self {
            Self::Typed(held) => Some(held.as_any()),
            Self::Written { .. } => None,
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
    history: Option<Box<dyn Recorder>>,
    written: Vec<Vec<u8>>,
    references: RefCell<Option<(u64, Vec<Uuid>)>>,
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
            history: None,
            written: Vec::new(),
            references: RefCell::new(None),
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
        if let (true, Some(history), Some(content)) =
            (mine, &mut self.history, self.content.as_any())
        {
            history.record(content, &operation);
        }
        self.content.apply(&operation);
        self.revision += 1;
        self.log.push((operation, mine));
        if mine {
            self.taken += 1;
        }
    }

    fn gesture(&mut self, phase: GesturePhase) {
        let Some(history) = &mut self.history else {
            return;
        };
        let reverted = match phase {
            GesturePhase::Begin => {
                history.begin();
                Vec::new()
            }
            GesturePhase::Commit => {
                history.commit();
                Vec::new()
            }
            GesturePhase::Cancel => match self.content.as_any() {
                Some(content) => history.cancel(content),
                None => Vec::new(),
            },
        };
        for operation in reverted {
            self.written.push(operation.clone());
            self.operate(operation, false);
        }
    }

    fn step_history(&mut self, redo: bool) {
        let (Some(history), Some(content)) = (&mut self.history, self.content.as_any()) else {
            return;
        };
        for operation in history.step(content, redo) {
            self.written.push(operation.clone());
            self.operate(operation, false);
        }
    }

    fn references(&self, organization: Uuid) -> Option<Vec<Uuid>> {
        let Stored::Typed(content) = &self.content else {
            return None;
        };
        let mut cached = self.references.borrow_mut();
        if let Some((revision, references)) = &*cached
            && *revision == self.revision
        {
            return Some(references.clone());
        }
        let references = content.references(organization);
        *cached = Some((self.revision, references.clone()));
        Some(references)
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
                        unsettled: 0,
                        beneath: None,
                    })
                    .collect(),
            },
            _ => EditorMessage::Content {
                instance,
                block_id: block.into_bytes(),
                content_type: self.content.content_type().into_bytes(),
                bytes: self.content.bytes(),
                session: self.content.session_state(),
                unsettled: Vec::new(),
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
    organization: Uuid,
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
    pub(crate) fn new(organization: Uuid) -> Self {
        let store = Self::default();
        store.0.borrow_mut().organization = organization;
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

    pub fn keep_history<C: Undo + 'static>(&self, block: Option<Uuid>) {
        let key = self.key(block);
        let mut inner = self.0.borrow_mut();
        let held = inner
            .blocks
            .get_mut(&key)
            .expect("the store holds that block");
        held.content.typed::<C>();
        held.history = Some(Box::new(History::<C>::default()));
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
        if let Some(held) = self.0.borrow_mut().blocks.get_mut(&key) {
            held.operate(operation, false);
        }
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
                    held.written.push(operation.clone());
                    held.operate(operation.clone(), true);
                }
            }
            EditorMessage::Gesture {
                block_id, phase, ..
            } => {
                if let Some(held) = self
                    .0
                    .borrow_mut()
                    .blocks
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    held.gesture(*phase);
                }
            }
            EditorMessage::BlockCommand {
                block_id,
                command: command @ (BlockCommand::Undo | BlockCommand::Redo),
                ..
            } => {
                if let Some(held) = self
                    .0
                    .borrow_mut()
                    .blocks
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    held.step_history(*command == BlockCommand::Redo);
                }
            }
            EditorMessage::ResendContent { block_id, .. } => {
                let mut inner = self.0.borrow_mut();
                if let Some(held) = inner.blocks.get_mut(&Uuid::from_bytes(*block_id)) {
                    held.sent = None;
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
                defined,
                content,
                ..
            } => {
                let id = Uuid::from_bytes(*block_id);
                let block_type = Uuid::from_bytes(*content_type);
                let info = BlockInfo::decode(block_plugin_api::BlockInfo {
                    block_id: *block_id,
                    block_type: *content_type,
                    author: Some(Uuid::nil().into_bytes()),
                    parent: Some(*parent),
                    references: Vec::new(),
                    access: block_plugin_api::AccessLevel::Edit,
                    metadata: block_plugin_api::BlockMetadata {
                        defined: defined.clone(),
                        ..block_plugin_api::BlockMetadata::default()
                    },
                    locked: false,
                    deleted: false,
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
                    info.parent = Some(BlockParent::decode(*parent));
                }
            }
            EditorMessage::Define {
                block_id,
                key,
                value,
                ..
            } => {
                if let Some(info) = self
                    .0
                    .borrow_mut()
                    .graph
                    .get_mut(&Uuid::from_bytes(*block_id))
                {
                    info.metadata.define(&[(
                        Uuid::from_bytes(*key),
                        value.as_ref().map(|value| value.to_vec()),
                    )]);
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

    fn held(&self, nodes: &[BlockInfo], node: &BlockInfo) -> bool {
        match node.parent {
            Some(BlockParent::Root) | None => true,
            Some(BlockParent::Detached) => false,
            Some(BlockParent::Block(parent)) => {
                !nodes.iter().any(|above| above.id == parent)
                    || self.references_of(parent).contains(&node.id)
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
                Some(BlockParent::Root | BlockParent::Detached) | None => return false,
            }
        }
        false
    }

    fn references_of(&self, id: Uuid) -> Vec<Uuid> {
        let inner = self.0.borrow();
        inner
            .blocks
            .get(&id)
            .and_then(|held| held.references(inner.organization))
            .or_else(|| inner.graph.get(&id).map(|info| info.references.clone()))
            .unwrap_or_default()
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
                .filter(|node| self.references_of(node.id).contains(&block))
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
