use block_editor_beui::be_block::display_settings::{DisplayMode, Rotation, ScreenOff};
use block_editor_beui::be_block::{
    DisplaySettings, DisplaySettingsContent, Root, Settings, SettingsContent, SettingsProfile,
    SettingsProfileContent, ThemeKind,
};
use block_editor_beui::beui::native::Document;
use block_editor_beui::beui::{Key, Modifiers, NodeId, Vec2};
use block_editor_beui::{
    BlockInfo, BlockParent, DisplaysWatch, EditorHost, HostDisplay, OwnedEditor, PowerAvailability,
    PowerWatch,
};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use crate::display::DisplaySettingsApp;

mod a_change_here_goes_to_every_device_and_a_toast_says_so;
mod a_profile_shows_the_theme_it_inherits;
mod a_screen_known_by_its_connector_is_this_devices_alone;
mod choosing_a_keyboard_height_keeps_it_on_this_device;
mod choosing_a_refresh_rate_stores_the_mode;
mod choosing_a_resolution_uses_its_fastest_refresh_rate;
mod choosing_a_rotation_stores_it_and_the_turned_screen_marks_it_now;
mod choosing_a_scale_stores_it;
mod choosing_a_theme_stores_it;
mod choosing_never_keeps_the_screens_on;
mod each_tile_is_drawn_turned_from_the_rotation_shown_now;
mod keeping_a_setting_copies_it_here_and_later_changes_stay_here;
mod keeping_makes_this_devices_display_block_when_it_has_none;
mod outside_a_session_only_the_appearance_is_shown;
mod resetting_a_display_returns_it_to_its_default;
mod resetting_a_kept_setting_clears_it_here;
mod the_lock_follows_the_screens_until_given_a_time_of_its_own;
mod the_rotate_button_in_the_bar_is_turned_on_and_reset;
mod the_rotations_are_named_from_how_the_panel_is_built;
mod the_toast_keeps_every_change_on_this_device_only;
mod the_toast_undoes_every_change;

const SESSION_HEIGHT: f32 = 1000.0;
const MONITOR: &str = "DEL|DELL AW2524H|7XQ2B34";
const THEME: &str = "display.theme";
const SCALE: &str = "display.scale";
const KEYBOARD: &str = "display.keyboard";
const RESOLUTION: &str = "display.DP-1.resolution";
const REFRESH: &str = "display.DP-1.refresh";
const SCREEN: &str = "display.DP-1";
const ROTATION: &str = "display.DP-1.rotation";
const ROTATE_IN_BAR: &str = "display.rotate-in-bar";
const SCREEN_OFF: &str = "display.screen-off";
const LOCK_AFTER: &str = "display.lock-after";

struct Harness {
    editor: BeuiTest<DisplaySettingsApp>,
    block: Uuid,
    client: Uuid,
    profile: Uuid,
    here: Option<Uuid>,
}

impl Harness {
    fn new() -> Self {
        Self::with_host(EditorHost::default())
    }

    fn in_a_session(displays: Vec<HostDisplay>) -> Self {
        let host = EditorHost::default();
        host.set_watched::<DisplaysWatch>(&displays);
        host.set_watched::<PowerWatch>(&PowerAvailability::default());
        let mut harness = Self::with_host(host);
        harness.editor.set_size(Vec2::new(800.0, SESSION_HEIGHT));
        harness
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
            profile: Uuid::new_v4(),
            here: None,
        };
        harness.editor.hold(None, DisplaySettingsContent::default());
        harness.run();
        harness
    }

    fn in_a_profile_over(mut self, defaults: ThemeKind) -> Self {
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

    fn on_this_device(self) -> Self {
        self.on_this_device_with(Some(DisplaySettingsContent::default()))
    }

    fn on_this_device_with(mut self, here: Option<DisplaySettingsContent>) -> Self {
        let store = self.editor.store();
        let settings = Uuid::new_v4();
        store.add_block(BlockInfo::new(
            settings,
            Settings::CONTENT_TYPE,
            BlockParent::Root,
        ));
        store.add_block(BlockInfo::new(
            self.profile,
            SettingsProfile::CONTENT_TYPE,
            BlockParent::Block(settings),
        ));
        let mut held = SettingsContent::default();
        held.apply(&Settings::set_kind(
            DisplaySettings::CONTENT_TYPE,
            self.block,
        ));
        held.apply(&Settings::add_profile(self.client, self.profile));
        store.hold(Some(settings), held);
        let mut profile = SettingsProfileContent::default();
        if let Some(content) = here {
            let block = Uuid::new_v4();
            profile.apply(&SettingsProfile::set_kind(
                DisplaySettings::CONTENT_TYPE,
                block,
            ));
            store.hold(Some(block), content);
            self.here = Some(block);
        }
        store.hold(Some(self.profile), profile);
        self.run();
        self
    }

    fn here(&self) -> DisplaySettings {
        let block = self.here.or_else(|| {
            self.editor
                .content::<SettingsProfileContent>(Some(self.profile))
                .root()
                .kind(DisplaySettings::CONTENT_TYPE)
        });
        block.map_or_else(DisplaySettings::default, |block| {
            self.editor
                .content::<DisplaySettingsContent>(Some(block))
                .root()
        })
    }

    fn every_device(&self) -> DisplaySettings {
        self.content().root()
    }

    fn open_menu(&mut self, row: &str) {
        self.editor.click(format!("{row}.menu"));
        self.run();
    }

    fn keep(&mut self, row: &str) {
        self.open_menu(row);
        self.editor.click(format!("{row}.keep"));
        self.run();
    }

    fn reset(&mut self, row: &str) {
        self.open_menu(row);
        self.editor.click(format!("{row}.reset"));
        self.run();
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
        self.editor.key(Key::Home, Modifiers::NONE);
        self.run();
        for _ in 0..index {
            self.editor.key(Key::ArrowDown, Modifiers::NONE);
            self.run();
        }
        self.editor.key(Key::Enter, Modifiers::NONE);
        self.run();
    }

    fn says(&self, words: &str) -> bool {
        let document = self.editor.document();
        document
            .root()
            .is_some_and(|root| text_within(&document, root, words))
    }
}

fn turned(rotation: Rotation) -> HostDisplay {
    HostDisplay {
        rotation,
        ..gaming_monitor()
    }
}

fn tile(rotation: Rotation) -> String {
    format!("display.DP-1.rotation.{}", rotation.degrees())
}

fn turns_within(document: &Document, tile: &str) -> Option<u8> {
    let screen = document.find_test_id(&format!("{tile}.screen"))?;
    let (_, rotate) = document
        .node_properties(screen)
        .into_iter()
        .find(|(name, _)| *name == "rotate")?;
    let quarters = rotate.parse::<f32>().ok()? / std::f32::consts::FRAC_PI_2;
    Some(quarters.round() as u8 % 4)
}

fn text_within(document: &Document, id: NodeId, words: &str) -> bool {
    document
        .arena
        .kind_of(id)
        .is_some_and(|node| document.text(node).contains(words))
        || document
            .children(id)
            .into_iter()
            .any(|child| text_within(&document, child, words))
}

fn mode(width: u32, height: u32, refresh_millihertz: u32) -> DisplayMode {
    DisplayMode {
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
        rotation: Rotation::Normal,
        rect: block_editor_beui::Rect::ZERO,
        scale: 1.0,
    }
}
