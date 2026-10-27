use super::*;

#[test]
fn a_focused_window_says_how_to_come_back_and_shades_what_is_off_screen() {
    painting(
        "a_focused_windows_window_says_how_to_come_back",
        None,
        Some(Hint {
            text: focused_hint(EntryKind::Window, &HotkeyState::Registered(Hotkey::DEFAULT)),
            focused: true,
        }),
        vec![part(0.0, 0.6, 1.0, 1.0), part(0.75, 0.0, 1.0, 0.6)],
    );
    painting(
        "a_focused_windows_window_without_the_hotkey_says_why_the_mouse_is_free",
        None,
        Some(Hint {
            text: focused_hint(
                EntryKind::Window,
                &HotkeyState::Refused(HotkeyError::Taken(Hotkey::DEFAULT)),
            ),
            focused: true,
        }),
        Vec::new(),
    );
}
