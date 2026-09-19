use std::sync::Arc;

use block_client::blocks::calendar::{Calendar, CalendarEvent};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CalendarApp;

mod adding_an_event_writes_it_to_the_block;
mod clicking_a_day_opens_the_new_event_form;

fn editor() -> (BeuiTest<CalendarApp>, BlockHandle<Calendar>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Calendar::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
}
