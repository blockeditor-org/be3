use block_editor_beui::be_block::DisplaySettingsContent;
use block_editor_beui::be_block::display_settings::{DisplayMode, ScreenOff};
use block_editor_beui::beui::Key;
use block_editor_beui::{Editor, EditorHost, HostDisplay, HostDisplayMode};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::DisplaySettingsApp;

mod choosing_a_refresh_rate_stores_the_mode;
mod choosing_a_resolution_uses_its_fastest_refresh_rate;
mod choosing_never_keeps_the_screens_on;
mod resetting_a_display_returns_it_to_its_default;
mod the_lock_follows_the_screens_until_given_a_time_of_its_own;

const MONITOR: &str = "DEL|DELL AW2524H|7XQ2B34";
const RESOLUTION: &str = "display-settings.DP-1.resolution";
const REFRESH: &str = "display-settings.DP-1.refresh";
const RESET: &str = "display-settings.DP-1.reset";
const SCREEN_OFF: &str = "display-settings.screen-off";
const LOCK_AFTER: &str = "display-settings.lock-after";

fn editor(displays: Vec<HostDisplay>) -> BeuiTest<DisplaySettingsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_displays(displays);
    let mut editor = BeuiTest::new(Editor::new(host, Uuid::new_v4()));
    editor.hold(None, DisplaySettingsContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<DisplaySettingsApp>) -> DisplaySettingsContent {
    editor.content(None)
}

fn choose(editor: &mut BeuiTest<DisplaySettingsApp>, select: &str, index: usize) {
    editor.click(select);
    editor.run();
    editor.key_press(Key::Home);
    editor.run();
    for _ in 0..index {
        editor.key_press(Key::ArrowDown);
        editor.run();
    }
    editor.key_press(Key::Enter);
    editor.run();
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
    }
}
