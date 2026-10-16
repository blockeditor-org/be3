use block_editor_beui::be_block::InputRemapperContent;
use block_editor_beui::beui::Vec2;
use block_editor_beui::{EditorHost, OwnedEditor, RemapState, Remaps};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::InputRemapsApp;

mod remaps_that_do_not_read_say_why_and_store_nothing;
mod remaps_written_as_text_name_this_plugins_remapper_once_applied;

const SIZE: Vec2 = Vec2::new(800.0, 760.0);

fn editor(state: RemapState) -> BeuiTest<InputRemapsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_host_value::<Remaps>(&state);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4())).with_size(SIZE);
    editor.hold(None, InputRemapperContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputRemapsApp>) -> InputRemapperContent {
    editor.content(None)
}

fn apply(editor: &mut BeuiTest<InputRemapsApp>, text: &str) {
    editor.click("input-remaps.text");
    editor.run();
    editor.text(text);
    editor.run();
    editor.click("input-remaps.apply");
    editor.run();
}
