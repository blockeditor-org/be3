use super::*;
use crate::events::*;

#[test]
fn win_events_about_whole_windows_become_events() {
    let window = Handle(42);
    assert_eq!(
        Event::from_win_event(EVENT_OBJECT_CREATE, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::Created(window))
    );
    assert_eq!(
        Event::from_win_event(EVENT_OBJECT_NAMECHANGE, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::NameChanged(window))
    );
    assert_eq!(
        Event::from_win_event(EVENT_OBJECT_LOCATIONCHANGE, window, -9, CHILDID_SELF),
        None
    );
    assert_eq!(
        Event::from_win_event(EVENT_OBJECT_NAMECHANGE, window, OBJID_WINDOW, 3),
        None
    );
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_FOREGROUND, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::Foreground(window))
    );
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_DESKTOPSWITCH, Handle(0), 0, 0),
        Some(Event::DesktopSwitched)
    );
    assert_eq!(
        Event::from_win_event(EVENT_OBJECT_CLOAKED, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::Cloaked(window))
    );
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_CAPTURESTART, Handle(7), OBJID_WINDOW, CHILDID_SELF),
        Some(Event::CaptureChanged)
    );
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_CAPTUREEND, Handle(0), 0, 0),
        Some(Event::CaptureChanged)
    );
    assert!(is_any_window_event(EVENT_SYSTEM_CAPTURESTART));
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_MOVESIZEEND, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::MoveSizeEnded(window))
    );
    assert_eq!(
        Event::from_win_event(EVENT_SYSTEM_MINIMIZEEND, window, OBJID_WINDOW, CHILDID_SELF),
        Some(Event::Restored(window))
    );
    assert_eq!(
        Event::from_win_event(0x8005, window, OBJID_WINDOW, CHILDID_SELF),
        None
    );
}
