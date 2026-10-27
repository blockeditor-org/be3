use be_win32::{Control, Entry, EntryId, EntryKind, Handle, InputDesktop, Monitor, Rect, Snapshot};
use block_plugin_api::{Pos2, Vec2};

use super::*;

fn snapshot(entries: Vec<Entry>) -> Snapshot {
    Snapshot {
        revision: 1,
        entries,
        monitors: vec![Monitor {
            handle: Handle(7),
            device: "\\\\.\\DISPLAY1".to_owned(),
            name: "Display".to_owned(),
            rect: Rect::new(0, 0, 2000, 1000),
            work_area: Rect::new(0, 0, 2000, 960),
            scale: 2.0,
            primary: true,
        }],
        input_desktop: InputDesktop::Default,
    }
}

fn window(id: u64, frame: Rect) -> Entry {
    Entry {
        id: EntryId(id),
        kind: EntryKind::Window,
        handle: Handle(id * 10),
        frame,
        responding: true,
        ..Entry::default()
    }
}

mod a_resized_panel_resizes_its_window_and_the_answer_is_not_its_own_size;
mod a_window_that_resizes_itself_is_followed;
mod displays_minimized_maximized_and_elevated_windows_are_never_resized;
mod fullscreen_maximizes_and_ends_when_the_window_is_restored;
