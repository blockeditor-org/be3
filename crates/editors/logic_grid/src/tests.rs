use std::sync::Arc;

use block::Block;
use block_client::blocks::compiled_logic::CompiledLogic;
use block_client::blocks::logic_grid::LogicGrid;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::beui::{Key, Pos2, Vec2};
use block_editor_plugin::{Artifact, Artifacts, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::LogicGridApp;

mod a_number_key_picks_a_tool_and_a_click_places_a_gate;
mod dragging_a_hotbar_slot_onto_another_moves_it_there;
mod dragging_with_the_wire_tool_draws_a_wire;
mod dropping_a_hotbar_slot_on_an_open_folder_puts_it_inside;
mod the_rename_setting_writes_the_artifact_draft;

fn editor() -> (BeuiTest<LogicGridApp>, BlockHandle<LogicGrid>) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(LogicGrid::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, client, block.id());
    let mut editor = BeuiTest::new(editor);
    editor.run();
    editor.run();
    (editor, block)
}

fn canvas_point(editor: &BeuiTest<LogicGridApp>, offset: Vec2) -> Pos2 {
    editor.rect_of("logic-grid.canvas").center() + offset
}
