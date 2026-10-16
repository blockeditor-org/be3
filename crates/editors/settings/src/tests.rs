use block_editor_beui::be_block::{
    BlockContent, DisplaySettingsContent, InputSettingsContent, UiSettingsContent,
};

use block_editor_beui::be_block::SettingsContent;
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::SettingsApp;

mod another_accounts_settings_are_shown_but_not_changed;
mod opening_display_settings_creates_the_block_once;
mod opening_input_settings_creates_the_block_once;
mod opening_ui_settings_creates_the_block_once;
mod resetting_a_kind_asks_before_deleting_its_block;
mod this_device_can_pick_a_new_settings_profile;

fn editor() -> BeuiTest<SettingsApp> {
    editor_for(Uuid::nil())
}

fn editor_for(account: Uuid) -> BeuiTest<SettingsApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_account_id(account);
    host.set_block_type(SettingsContent::CONTENT_TYPE);
    host.set_editable(true);
    host.set_client_id(Uuid::new_v4());
    let editor = OwnedEditor::new(host.clone(), block);
    let mut editor = BeuiTest::new(editor);
    editor.hold(None, SettingsContent::default());
    editor.run();
    editor
}

fn entries(editor: &BeuiTest<SettingsApp>, block_type: Uuid) -> Vec<Uuid> {
    let store = editor.store();
    let created: Vec<Uuid> = store
        .created()
        .into_iter()
        .filter(|block| {
            store
                .block(*block)
                .is_some_and(|info| info.block_type == block_type)
        })
        .collect();
    let kind = editor
        .content::<SettingsContent>(None)
        .root()
        .kind(block_type);
    assert!(
        created.iter().all(|block| kind == Some(*block)),
        "every block made is the one the settings point at"
    );
    created
}
