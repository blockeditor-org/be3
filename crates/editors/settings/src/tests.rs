use block_editor_plugin::be_block::{BlockContent, UiSettingsContent};

use block_editor_plugin::be_block::SettingsContent;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::SettingsApp;

mod opening_ui_settings_creates_the_block_once;

fn editor() -> ContentHarness<SettingsApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let editor = Editor::new(host.clone(), block);
    let mut editor = ContentHarness::new(BeuiTest::new(editor), host);
    editor.hold(None, SettingsContent::default());
    editor.run();
    editor
}

fn entries(editor: &ContentHarness<SettingsApp>) -> Vec<Uuid> {
    editor
        .content::<SettingsContent>(None)
        .root()
        .entries
        .iter()
        .filter(|((block_type, _), _)| *block_type == UiSettingsContent::CONTENT_TYPE)
        .map(|(_, block)| *block)
        .collect()
}
