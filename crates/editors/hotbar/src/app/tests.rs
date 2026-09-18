use std::sync::Arc;

use block_client::blocks::hotbar::Hotbar;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use super::*;
use crate::app::HotbarApp;

mod unpinning_a_component_from_the_hotbar_takes_its_row_away;
mod unpinning_a_component_removes_it_from_every_folder;

fn editor(slots: Vec<HotbarSlot>) -> (BeuiTest<HotbarApp>, BlockHandle<Hotbar>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Hotbar::with_slots(slots));
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    (editor, block)
}
