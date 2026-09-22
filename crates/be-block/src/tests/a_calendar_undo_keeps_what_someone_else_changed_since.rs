use super::*;

#[test]
fn a_calendar_undo_keeps_what_someone_else_changed_since() {
    let event = meeting();
    let mut calendar = scheduled(&event);
    let renamed = CalendarEvent {
        title: "Retro".to_owned(),
        ..event.clone()
    };
    let rename = CalendarOp::UpdateEvent { event: renamed };
    let step = calendar.step(&rename).expect("a rename is undoable");
    calendar.apply(&rename);

    let moved = CalendarEvent {
        start: 600,
        end: 630,
        ..calendar.event(event.id).unwrap().clone()
    };
    calendar.apply(&CalendarOp::UpdateEvent { event: moved });
    for operation in calendar.revert(&step) {
        calendar.apply(&operation);
    }

    let shown = calendar.event(event.id).unwrap();
    assert_eq!(
        (shown.title.as_str(), shown.start, shown.end),
        ("Standup", 600, 630)
    );
    for operation in calendar.reapply(&step) {
        calendar.apply(&operation);
    }
    assert_eq!(calendar.event(event.id).unwrap().title, "Retro");

    let removed = calendar
        .step(&CalendarOp::RemoveEvent { id: event.id })
        .unwrap();
    calendar.apply(&CalendarOp::RemoveEvent { id: event.id });
    for operation in calendar.revert(&removed) {
        calendar.apply(&operation);
    }
    assert!(calendar.event(event.id).is_some());
    assert_eq!(
        calendar.step(&CalendarOp::RemoveEvent { id: Uuid::new_v4() }),
        None
    );
}
