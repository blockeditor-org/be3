use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use calendar_types::CalendarContent;
use uuid::Uuid;

use crate::app::CalendarApp;

mod adding_an_event_writes_it_to_the_block;
mod an_event_opens_in_a_form_wide_enough_for_its_fields_and_actions;
mod clicking_a_day_opens_the_new_event_form;
mod moving_the_start_of_an_event_keeps_its_length;

struct Harness {
    editor: BeuiTest<CalendarApp>,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        let editor = OwnedEditor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
        };
        harness
            .editor
            .store()
            .register(calendar_types::registrations());
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
