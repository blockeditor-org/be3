use super::*;

#[test]
fn the_hints_name_the_way_back() {
    let registered = HotkeyState::Registered(Hotkey::DEFAULT);
    assert_eq!(
        focused_hint(EntryKind::Window, &registered),
        "Windows has the mouse and keyboard. Press Ctrl+Alt+Space to come back to Block."
    );
    assert_eq!(
        hover_hint(EntryKind::Window, &registered),
        "Click to use this window. Ctrl+Alt+Space comes back."
    );
    let taken = HotkeyState::Refused(HotkeyError::Taken(Hotkey::DEFAULT));
    assert_eq!(
        focused_hint(EntryKind::Window, &taken),
        "Ctrl+Alt+Space is taken by another program, so the mouse isn't kept in this window. Press Alt+Tab to come back to Block."
    );
    assert_eq!(
        hover_hint(EntryKind::Display, &taken),
        "Click to use this display. Alt+Tab comes back."
    );
    assert_eq!(
        focused_hint(EntryKind::Display, &HotkeyState::Waiting),
        "The shortcut back to Block isn't ready yet, so the mouse isn't kept in this display. Press Alt+Tab to come back to Block."
    );
    assert_eq!(
        ended_notice(be_win32::EndReason::NeverArrived),
        Some(SurfaceNotice::Focus(FocusError::ForegroundRefused))
    );
    assert_eq!(ended_notice(be_win32::EndReason::Hotkey), None);
    assert_eq!(refused_notice(&FocusError::Returned), None);
    assert_eq!(
        refused_notice(&FocusError::OffScreen),
        Some(SurfaceNotice::Focus(FocusError::OffScreen))
    );
}
