use block_editor_plugin::be_block::{
    BlockContent, Counter as CounterModel, CounterContent, LiveEdit,
};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CounterApp;

mod an_edit_stays_on_screen_until_the_host_takes_it;
mod clicking_the_plus_button_counts_up_on_the_block;
mod resetting_puts_the_block_back_to_zero;
mod the_counter_shows_what_the_block_holds;

struct Harness {
    editor: BeuiTest<CounterApp>,
    host: EditorHost,
    content: CounterContent,
    applied: u64,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host.clone(), block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            host,
            content: CounterContent::default(),
            applied: 0,
        };
        harness.publish();
        harness.run();
        harness
    }

    fn click(&mut self, test_id: &str) {
        self.editor.click(test_id);
    }

    fn run(&mut self) {
        self.editor.run();
        let mut changed = false;
        for operation in self.host.take_content_operations() {
            let operation = CounterContent::decode_operation(&operation)
                .expect("the editor sent an operation the counter cannot read");
            self.content.apply(&operation);
            self.applied += 1;
            changed = true;
        }
        if changed {
            self.publish();
        }
        self.editor.run();
    }

    fn publish(&mut self) {
        self.host.set_block_content(
            CounterContent::CONTENT_TYPE,
            self.content.encode(),
            self.applied,
        );
    }

    fn set_count(&mut self, count: i64) {
        let by = count - self.count();
        self.content.apply(&CounterModel::add(by));
        self.publish();
    }

    fn count(&self) -> i64 {
        self.content.root().value()
    }

    fn shown(&mut self) -> String {
        let value = self
            .editor
            .document()
            .find_test_id("counter.value")
            .expect("the ui has no counter.value node");
        self.editor
            .document()
            .node_detail(value)
            .expect("the value node has no text")
    }
}
