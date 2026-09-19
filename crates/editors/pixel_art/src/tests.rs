use std::sync::Arc;

use block_client::blocks::pixel_art::{PixelArt, PixelColor};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::beui::Pos2;
use block_editor_plugin::beui::styled::toggle_button_pressed;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::PixelArtApp;

mod choosing_a_tool_marks_it_in_the_sidebar;
mod clearing_the_artwork_needs_the_dialog;
mod drawing_on_the_canvas_paints_the_block;

fn editor() -> (BeuiTest<PixelArtApp>, BlockHandle<PixelArt>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(PixelArt::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
}
