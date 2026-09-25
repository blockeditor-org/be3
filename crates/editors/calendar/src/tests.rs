use block_editor_beui::be_block::{BlockContent, CalendarContent, LiveEdit};
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CalendarApp;

mod adding_an_event_writes_it_to_the_block;
mod clicking_a_day_opens_the_new_event_form;

struct Harness {
    editor: BeuiTest<CalendarApp>,
    host: EditorHost,
    content: CalendarContent,
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
            content: CalendarContent::default(),
            applied: 0,
        };
        harness.publish();
        harness.run();
        harness
    }

    fn run(&mut self) {
        self.editor.run();
        let mut changed = false;
        for operation in self.host.take_content_operations() {
            let operation = CalendarContent::decode_operation(&operation)
                .expect("the editor sent an operation the calendar cannot read");
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
            CalendarContent::CONTENT_TYPE,
            self.content.encode(),
            self.applied,
        );
    }
}
