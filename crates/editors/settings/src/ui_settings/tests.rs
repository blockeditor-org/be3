use block_editor_beui::be_block::{
    Root, Settings, SettingsContent, SettingsProfile, SettingsProfileContent, Theme, UiSettings,
    UiSettingsContent,
};
use block_editor_beui::beui::{Document, Key, NodeId};
use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::ui_settings::UiSettingsApp;

mod a_profile_shows_the_theme_it_inherits;
mod choosing_a_default_editor_stores_it;
mod choosing_a_scale_stores_it;
mod choosing_a_theme_stores_it;

struct Harness {
    editor: BeuiTest<UiSettingsApp>,
    block: Uuid,
    client: Uuid,
}

impl Harness {
    fn new() -> Self {
        let block = Uuid::new_v4();
        let client = Uuid::new_v4();
        let host = EditorHost::default();
        host.set_editable(true);
        host.set_client_id(client);
        let editor = OwnedEditor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            block,
            client,
        };
        harness.editor.hold(None, UiSettingsContent::default());
        harness.run();
        harness
    }

    fn in_a_profile_over(mut self, defaults: Theme) -> Self {
        let store = self.editor.store();
        let (settings, profile, shared) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        store.add_block(BlockInfo::new(
            settings,
            Settings::CONTENT_TYPE,
            BlockParent::Root,
        ));
        let mut held = SettingsContent::default();
        held.apply(&Settings::set_kind(UiSettings::CONTENT_TYPE, shared));
        held.apply(&Settings::add_profile(self.client, profile));
        store.hold(Some(settings), held);
        let mut own = SettingsProfileContent::default();
        own.apply(&SettingsProfile::set_kind(
            UiSettings::CONTENT_TYPE,
            self.block,
        ));
        store.hold(Some(profile), own);
        let mut lower = UiSettingsContent::default();
        lower.apply(&UiSettings::set_theme(Some(defaults)));
        store.hold(Some(shared), lower);
        self.run();
        self
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn content(&self) -> UiSettingsContent {
        self.editor.content(None)
    }

    fn choose(&mut self, index: usize) {
        self.choose_in("ui-settings.theme", index);
    }

    fn choose_in(&mut self, select: &str, index: usize) {
        self.editor.click(select);
        self.run();
        self.editor.key_press(Key::Home);
        self.run();
        for _ in 0..index {
            self.editor.key_press(Key::ArrowDown);
            self.run();
        }
        self.editor.key_press(Key::Enter);
        self.run();
    }

    fn says(&self, words: &str) -> bool {
        let document = self.editor.document();
        document
            .root()
            .is_some_and(|root| text_within(document, root, words))
    }
}

fn text_within(document: &Document, id: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
        || document
            .children(id)
            .into_iter()
            .any(|child| text_within(document, child, words))
}
