use super::*;

#[test]
fn a_hovered_window_says_a_click_uses_it() {
    painting(
        "a_hovered_windows_window_says_a_click_uses_it",
        None,
        Some(Hint {
            text: hover_hint(EntryKind::Window, &HotkeyState::Registered(Hotkey::DEFAULT)),
            focused: false,
        }),
        Vec::new(),
    );
    painting(
        "a_windows_window_that_refused_focus_says_so",
        Some(SurfaceNotice::Focus(FocusError::ForegroundRefused)),
        None,
        Vec::new(),
    );
}
