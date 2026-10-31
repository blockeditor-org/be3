use std::sync::Arc;

use be_win32::{
    Entry, EntryId, EntryKind, FocusError, Hotkey, HotkeyError, Image, InputDesktop, Monitor,
    Origin, Point, Program, Rect, Shortcut, Snapshot,
};
use block_plugin_api::{HostWindowId, Pos2, Vec2, WindowKind};

use super::*;

fn monitor(device: &str, rect: Rect, scale: f32, primary: bool) -> Monitor {
    Monitor {
        handle: be_win32::Handle(7),
        device: device.to_owned(),
        name: format!("{device} name"),
        rect,
        work_area: rect,
        scale,
        primary,
    }
}

fn snapshot(entries: Vec<Entry>) -> Snapshot {
    Snapshot {
        revision: 1,
        entries,
        monitors: vec![
            monitor("\\\\.\\DISPLAY1", Rect::new(0, 0, 2000, 1000), 2.0, true),
            monitor("\\\\.\\DISPLAY2", Rect::new(2000, 0, 3000, 800), 1.0, false),
        ],
        input_desktop: InputDesktop::Default,
    }
}

fn window(id: u64, frame: Rect) -> Entry {
    Entry {
        id: EntryId(id),
        kind: EntryKind::Window,
        title: format!("window {id}"),
        app_id: "C:\\Programs\\paint.exe".to_owned(),
        frame,
        responding: true,
        ..Entry::default()
    }
}

mod a_failed_capture_restarts_once_then_tells_the_person;
mod a_point_in_a_panel_lands_on_the_same_place_in_the_frame;
mod a_popup_sits_at_its_offset_from_its_window_in_the_panels_scale;
mod a_start_menu_shortcut_becomes_a_program;
mod a_window_covering_a_monitor_or_maximized_by_the_shell_fills_the_shell;
mod a_window_entry_becomes_a_host_window_in_its_monitors_points;
mod a_window_is_named_after_its_start_menu_program;
mod displays_come_first_as_display_entries_and_keep_whether_they_are_shown;
mod entry_ids_are_host_window_ids;
mod hidden_windows_are_left_out_and_never_parents;
mod only_a_click_on_the_content_starts_focus_mode;
mod power_maps_both_ways_and_windows_never_ends_the_session_itself;
mod the_note_names_the_way_back_and_what_was_refused;
mod the_parts_of_a_window_off_every_monitor_are_shaded;
mod the_secure_desktop_covers_displays_and_refusals_are_only_real_ones;
