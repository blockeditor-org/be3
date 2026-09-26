use block_editor_plugin::be_block::CalendarContent;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CalendarApp;

mod adding_an_event_writes_it_to_the_block;
mod clicking_a_day_opens_the_new_event_form;

struct Harness {
    editor: BeuiTest<CalendarApp>,
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
        harness.editor.hold(None, CalendarContent::default());
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn content(&self) -> CalendarContent {
        self.editor.content(None)
    }
}
