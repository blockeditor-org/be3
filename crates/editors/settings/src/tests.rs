use std::sync::Arc;

use block::Block;
use block_client::BlockClient;
use block_client::blocks::settings::Settings;
use block_client::blocks::ui_settings::UiSettings;
use block_editor_plugin::be_block::SettingsContent;
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::SettingsApp;

mod opening_ui_settings_creates_the_block_once;

fn editor() -> ContentHarness<SettingsApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(Settings);
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let editor = Editor::new(host.clone(), client, block.id());
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
        .filter(|((block_type, _), _)| *block_type == UiSettings::TYPE_ID)
        .map(|(_, block)| *block)
        .collect()
}
