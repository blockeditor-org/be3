use block_editor_beui::be_block::display_settings::{DisplayMode, ScreenOff};
use block_editor_beui::be_block::{
    DisplaySettings, DisplaySettingsContent, Root, Settings, SettingsContent, SettingsProfile,
    SettingsProfileContent, Theme,
};
use block_editor_beui::beui::{Document, Key, NodeId};
use block_editor_beui::{
    BlockInfo, BlockParent, Displays, EditorHost, HostDisplay, HostDisplayMode, HostRotation, OwnedEditor, Power,
    PowerAvailability,
};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::display::DisplaySettingsApp;

mod a_profile_shows_the_theme_it_inherits;
mod choosing_a_refresh_rate_stores_the_mode;
mod choosing_a_resolution_uses_its_fastest_refresh_rate;
mod choosing_a_scale_stores_it;
mod choosing_a_theme_stores_it;
mod choosing_never_keeps_the_screens_on;
mod outside_a_session_only_the_appearance_is_shown;
mod resetting_a_display_returns_it_to_its_default;
mod the_lock_follows_the_screens_until_given_a_time_of_its_own;

const MONITOR: &str = "DEL|DELL AW2524H|7XQ2B34";
const THEME: &str = "display.theme";
const SCALE: &str = "display.scale";
const RESOLUTION: &str = "display.DP-1.resolution";
const REFRESH: &str = "display.DP-1.refresh";
const RESET: &str = "display.DP-1.reset";
const SCREEN_OFF: &str = "display.screen-off";
const LOCK_AFTER: &str = "display.lock-after";

struct Harness {
    editor: BeuiTest<DisplaySettingsApp>,
    block: Uuid,
    client: Uuid,
}

impl Harness {
    fn new() -> Self {
        Self::with_host(EditorHost::default())
    }

    fn in_a_session(displays: Vec<HostDisplay>) -> Self {
        let host = EditorHost::default();
        host.set_host_value::<Displays>(&displays);
        host.set_host_value::<Power>(&PowerAvailability::default());
        Self::with_host(host)
    }

    fn with_host(host: EditorHost) -> Self {
        let block = Uuid::new_v4();
        let client = Uuid::new_v4();
        host.set_editable(true);
        host.set_client_id(client);
        let editor = OwnedEditor::new(host, block);
        let mut harness = Self {
            editor: BeuiTest::new(editor),
            block,
            client,
        };
        harness.editor.hold(None, DisplaySettingsContent::default());
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
        held.apply(&Settings::set_kind(DisplaySettings::CONTENT_TYPE, shared));
        held.apply(&Settings::add_profile(self.client, profile));
        store.hold(Some(settings), held);
        let mut own = SettingsProfileContent::default();
        own.apply(&SettingsProfile::set_kind(
            DisplaySettings::CONTENT_TYPE,
            self.block,
        ));
        store.hold(Some(profile), own);
        let mut lower = DisplaySettingsContent::default();
        lower.apply(&DisplaySettings::set_theme(Some(defaults)));
        store.hold(Some(shared), lower);
        self.run();
        self
    }

    fn run(&mut self) {
        self.editor.run();
    }

    fn content(&self) -> DisplaySettingsContent {
        self.editor.content(None)
    }

    fn choose(&mut self, select: &str, index: usize) {
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

fn mode(width: u32, height: u32, refresh_millihertz: u32) -> HostDisplayMode {
    HostDisplayMode {
        width,
        height,
        refresh_millihertz,
    }
}

fn saved(width: u32, height: u32, refresh_millihertz: u32) -> DisplayMode {
    DisplayMode {
        width,
        height,
        refresh_millihertz,
    }
}

fn gaming_monitor() -> HostDisplay {
    HostDisplay {
        id: MONITOR.to_owned(),
        name: "DELL AW2524H".to_owned(),
        connector: "DP-1".to_owned(),
        modes: vec![
            mode(2560, 1440, 239_970),
            mode(2560, 1440, 143_998),
            mode(2560, 1440, 59_951),
            mode(1920, 1080, 240_000),
            mode(1920, 1080, 60_000),
        ],
        default: mode(2560, 1440, 239_970),
        current: mode(2560, 1440, 59_951),
        rotation: HostRotation::Normal,
    }
}
