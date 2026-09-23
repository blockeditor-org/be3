use std::any::Any;
use std::collections::BTreeMap;

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

struct Block {
    content: Box<dyn Held>,
    applied: u64,
}

pub struct ContentHarness<A: BeuiApp> {
    pub editor: BeuiTest<A>,
    pub host: EditorHost,
    blocks: BTreeMap<Option<Uuid>, Block>,
    seeded: Vec<SeededContent>,
}

impl<A: BeuiApp> ContentHarness<A> {
    pub fn new(editor: BeuiTest<A>, host: EditorHost) -> Self {
        Self {
            editor,
            host,
            blocks: BTreeMap::new(),
            seeded: Vec::new(),
        }
    }

    pub fn hold<C: LiveEdit>(&mut self, block: Option<Uuid>, content: C) {
        self.blocks.insert(
            block,
            Block {
                content: Box::new(content),
                applied: 0,
            },
        );
        self.publish(block);
    }

    pub fn content<C: LiveEdit>(&self, block: Option<Uuid>) -> &C {
        self.blocks
            .get(&block)
            .and_then(|held| held.content.as_any().downcast_ref::<C>())
            .expect("the harness holds that block with that content type")
    }

    pub fn edit<C: LiveEdit>(&mut self, block: Option<Uuid>, operation: &C::Op) {
        self.blocks
            .get_mut(&block)
            .expect("the harness holds that block")
            .content
            .apply(&C::encode_operation(operation));
        self.publish(block);
        self.editor.run();
    }

    pub fn seeded(&mut self) -> Vec<SeededContent> {
        self.seeded.extend(self.host.take_seeded_content());
        self.seeded.clone()
    }

    pub fn run(&mut self) {
        self.editor.run();
        let keys: Vec<Option<Uuid>> = self.blocks.keys().copied().collect();
        let mut changed = Vec::new();
        for key in keys {
            let operations = match key {
                Some(block) => self.host.take_content_operations_of(block),
                None => self.host.take_content_operations(),
            };
            if operations.is_empty() {
                continue;
            }
            if let Some(held) = self.blocks.get_mut(&key) {
                for operation in &operations {
                    held.content.apply(operation);
                    held.applied += 1;
                }
            }
            changed.push(key);
        }
        for key in &changed {
            self.publish(*key);
        }
        if !changed.is_empty() {
            self.editor.run();
        }
    }

    fn publish(&self, block: Option<Uuid>) {
        let Some(held) = self.blocks.get(&block) else {
            return;
        };
        let (content_type, bytes) = (held.content.content_type(), held.content.encode());
        match block {
            Some(block) => self
                .host
                .set_content_of(block, content_type, bytes, held.applied),
            None => self
                .host
                .set_block_content(content_type, bytes, held.applied),
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
