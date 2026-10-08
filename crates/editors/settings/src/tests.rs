use block_editor_beui::be_block::{
    BlockContent, DisplaySettingsContent, InputSettingsContent, UiSettingsContent,
};

use block_editor_beui::be_block::SettingsContent;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::SettingsApp;

mod opening_display_settings_creates_the_block_once;
mod opening_input_settings_creates_the_block_once;
mod opening_ui_settings_creates_the_block_once;

fn editor() -> BeuiTest<SettingsApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, SettingsContent::default());
    editor.run();
    editor
}

fn entries(editor: &BeuiTest<SettingsApp>, block_type: Uuid) -> Vec<Uuid> {
    editor
        .content::<SettingsContent>(None)
        .root()
        .entries
        .iter()
        .filter(|((entry_type, _), _)| *entry_type == block_type)
        .map(|(_, block)| *block)
        .collect()
}
