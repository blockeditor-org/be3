use block_editor_plugin::be_block::{BlockContent, CompiledLogicContent};

use block_editor_plugin::be_block::LogicGridContent;
use block_editor_plugin::beui::{Key, Pos2, Vec2};
use block_editor_plugin::{Artifact, Artifacts, Editor, EditorHost};
use block_ui_test::BeuiTest;
use logicgame::grid::LogicGrid as Grid;
use uuid::Uuid;

use crate::app::LogicGridApp;

mod a_dragged_hotbar_slot_follows_the_pointer;
mod a_number_key_picks_a_tool_and_a_click_places_a_gate;
mod compiling_a_grid_seeds_the_component_it_creates;
mod dragging_a_hotbar_slot_onto_another_moves_it_there;
mod dragging_with_the_wire_tool_draws_a_wire;
mod dropping_a_hotbar_slot_on_an_open_folder_puts_it_inside;
mod the_rename_setting_writes_the_artifact_draft;

fn editor() -> BeuiTest<LogicGridApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, LogicGridContent::default());
    editor.run();
    editor.run();
    editor
}

fn grid(editor: &BeuiTest<LogicGridApp>) -> Grid {
    editor.content::<LogicGridContent>(None).root().grid()
}

fn canvas_point(editor: &BeuiTest<LogicGridApp>, offset: Vec2) -> Pos2 {
    editor.rect_of("logic-grid.canvas").center() + offset
}
