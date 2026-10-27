use be_win32::{Control, Entry, EntryId, EntryKind, Handle, InputDesktop, Rect, Snapshot};

use super::*;

fn entry(kind: EntryKind, control: Control) -> Entry {
    Entry {
        id: EntryId(1),
        kind,
        handle: Handle(42),
        frame: Rect::new(10, 20, 110, 220),
        control,
        ..Entry::default()
    }
}

mod an_elevated_window_or_the_secure_desktop_takes_no_injected_input;
