use block_editor_beui::be_block::settings::{Settings, SettingsProfile};
use block_editor_beui::be_block::{
    BlockContent, DeviceClass, DisplaySettings, DisplaySettingsContent, InputSettingsContent,
    SettingsProfileContent,
};
use block_editor_beui::beui::{Key, Modifiers, Vec2};
use block_editor_beui::{BlockInfo, BlockParent};

use block_editor_beui::be_block::SettingsContent;
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::SettingsApp;

mod a_folders_settings_say_what_is_set_here_and_what_comes_from_above;
mod a_new_e_paper_tablet_is_set_up_with_only_the_settings_that_differ;
mod a_workspaces_home_and_prefixes_list_what_they_hold;
mod a_workspaces_tags_are_added_named_and_list_the_blocks_they_are_on;
mod another_accounts_settings_are_shown_but_not_changed;
mod changing_the_kind_of_device_keeps_its_defaults_here;
mod devices_and_profiles_shows_only_what_a_profile_keeps;
mod opening_display_settings_creates_the_block_once;
mod opening_input_settings_creates_the_block_once;
mod this_device_can_pick_a_new_settings_profile;
mod using_my_usual_settings_removes_what_the_kind_kept_here;

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
    editor.store().add_block(block_editor_beui::BlockInfo::new(
        block,
        SettingsContent::CONTENT_TYPE,
        block_editor_beui::BlockParent::Root,
    ));
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

struct SetUp {
    editor: BeuiTest<crate::app::SettingsProfileApp>,
}

impl SetUp {
    fn new(class: DeviceClass, every_device: DisplaySettings) -> Self {
        let (settings, profile, shared, client) = (
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        );
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(client);
        let mut editor = BeuiTest::new(OwnedEditor::new(host, profile));
        editor.set_size(Vec2::new(720.0, 640.0));
        let store = editor.store();
        store.add_block(BlockInfo::new(
            settings,
            SettingsContent::CONTENT_TYPE,
            BlockParent::Root,
        ));
        let mut named = BlockInfo::new(
            profile,
            SettingsProfileContent::CONTENT_TYPE,
            BlockParent::Block(settings),
        );
        named.set_name(class.name());
        store.add_block(named);
        let mut held = SettingsContent::default();
        held.apply(&Settings::set_kind(
            DisplaySettingsContent::CONTENT_TYPE,
            shared,
        ));
        held.apply(&Settings::add_profile(client, profile));
        store.hold(Some(settings), held);
        store.hold(Some(shared), DisplaySettingsContent::new(&every_device));
        let mut own = SettingsProfileContent::default();
        own.apply(&SettingsProfile::set_class(Some(class)));
        let display = class.display_settings();
        if display != DisplaySettings::default() {
            let here = Uuid::new_v4();
            own.apply(&SettingsProfile::set_kind(
                DisplaySettingsContent::CONTENT_TYPE,
                here,
            ));
            store.hold(Some(here), DisplaySettingsContent::new(&display));
        }
        editor.hold(None, own);
        editor.run();
        Self { editor }
    }

    fn profile(&self) -> SettingsProfile {
        self.editor.content::<SettingsProfileContent>(None).root()
    }

    fn here(&self) -> DisplaySettings {
        self.profile()
            .kind(DisplaySettingsContent::CONTENT_TYPE)
            .map_or_else(DisplaySettings::default, |block| {
                self.editor
                    .content::<DisplaySettingsContent>(Some(block))
                    .root()
            })
    }

    fn shows(&self, test_id: &str) -> bool {
        self.editor.document().find_test_id(test_id).is_some()
    }

    fn choose_kind(&mut self, class: DeviceClass) {
        let index = DeviceClass::ALL
            .iter()
            .position(|known| *known == class)
            .expect("every class is offered");
        self.editor.click("setup.kind");
        self.editor.run();
        self.editor.key(Key::Home, Modifiers::NONE);
        self.editor.run();
        for _ in 0..index {
            self.editor.key(Key::ArrowDown, Modifiers::NONE);
            self.editor.run();
        }
        self.editor.key(Key::Enter, Modifiers::NONE);
        self.editor.run();
    }
}
