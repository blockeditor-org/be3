use super::*;
use crate::events::{
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, OBJID_CURSOR,
    OBJID_WINDOW, is_cursor_event,
};

#[test]
fn the_cursor_names_the_window_and_display_under_it() {
    let mut main = entry(3, EntryKind::Window, Rect::new(2000, 100, 2600, 700));
    main.popups = vec![Popup {
        handle: Handle(99),
        frame: Rect::new(2500, 600, 2700, 800),
    }];
    let snapshot = desk(vec![
        entry(1, EntryKind::Display, Rect::new(0, 0, 1920, 1080)),
        entry(2, EntryKind::Display, Rect::new(1920, 0, 3840, 1080)),
        main,
    ]);
    let cursor = Cursor::locate(&snapshot, Point::new(2650, 750), true, Some(Handle(99)));
    assert_eq!(
        cursor,
        Cursor {
            position: Point::new(2650, 750),
            shown: true,
            window: Some(Over {
                entry: EntryId(3),
                point: Point::new(650, 650)
            }),
            display: Some(Over {
                entry: EntryId(2),
                point: Point::new(730, 750)
            }),
        }
    );
    let cursor = Cursor::locate(&snapshot, Point::new(10, 20), false, Some(Handle(1234)));
    assert_eq!(cursor.window, None);
    assert_eq!(
        cursor.display,
        Some(Over {
            entry: EntryId(1),
            point: Point::new(10, 20)
        })
    );
    assert!(!cursor.shown);

    assert!(is_cursor_event(EVENT_OBJECT_LOCATIONCHANGE, OBJID_CURSOR));
    assert!(is_cursor_event(EVENT_OBJECT_SHOW, OBJID_CURSOR));
    assert!(!is_cursor_event(EVENT_OBJECT_LOCATIONCHANGE, OBJID_WINDOW));
    assert!(!is_cursor_event(EVENT_OBJECT_NAMECHANGE, OBJID_CURSOR));
}
