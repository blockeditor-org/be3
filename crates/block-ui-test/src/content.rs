use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use block_editor_plugin::be_block::{BlockContent, LiveEdit};
use block_editor_plugin::{BeuiApp, EditorHost, SeededContent};
use uuid::Uuid;

use crate::BeuiTest;

trait Held: Any {
    fn apply(&mut self, operation: &[u8]);

    fn encode(&self) -> Vec<u8>;

    fn content_type(&self) -> Uuid;

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
    blocks: BTreeMap<Option<Uuid>, Block>,
    seeded: Vec<SeededContent>,
}

#[derive(Clone)]
pub struct ContentStore(Rc<RefCell<Inner>>);

impl ContentStore {
    pub fn new(host: EditorHost) -> Self {
        Self(Rc::new(RefCell::new(Inner {
            host,
            blocks: BTreeMap::new(),
            seeded: Vec::new(),
        })))
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
        !changed.is_empty()
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
        if self.store.sync() {
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
