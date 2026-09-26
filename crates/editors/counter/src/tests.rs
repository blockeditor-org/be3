use block_editor_beui::be_block::{Counter as CounterModel, CounterContent};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CounterApp;

mod an_edit_stays_on_screen_until_the_host_takes_it;
mod clicking_the_plus_button_counts_up_on_the_block;
mod resetting_puts_the_block_back_to_zero;
mod the_counter_shows_what_the_block_holds;

struct Harness {
    editor: BeuiTest<CounterApp>,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = Editor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
        };
        harness.editor.hold(None, CounterContent::default());
        harness.run();
        harness
    }

    fn click(&mut self, test_id: &str) {
        self.editor.click(test_id);
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn set_count(&mut self, count: i64) {
        let by = count - self.count();
        self.editor
            .edit::<CounterContent>(None, &CounterModel::add(by));
    }

    fn count(&self) -> i64 {
        self.editor.content::<CounterContent>(None).root().value()
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
