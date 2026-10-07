use block_editor_beui::be_block::InputSettingsContent;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::InputSettingsApp;

mod resetting_tap_to_click_returns_it_to_the_device_default;
mod switching_on_natural_scrolling_stores_it;
mod typing_a_layout_stores_it;

fn editor() -> BeuiTest<InputSettingsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::new(Editor::new(host, Uuid::new_v4()));
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputSettingsApp>) -> InputSettingsContent {
    editor.content(None)
}
