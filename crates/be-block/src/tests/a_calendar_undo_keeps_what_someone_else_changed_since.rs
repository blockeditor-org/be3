use super::*;

#[test]
fn a_calendar_undo_keeps_what_someone_else_changed_since() {
    let (calendar, id) = scheduled();
    let rename = calendar
        .root()
        .update(id, &CalendarEvent::new("Retro", 540, 555));
    let step = calendar.step(&rename).expect("a rename is undoable");
    let calendar = edited(&calendar, [rename]);

    let reschedule = calendar
        .root()
        .update(id, &CalendarEvent::new("Retro", 600, 630));
    let calendar = edited(&calendar, [reschedule]);
    let calendar = edited(&calendar, calendar.revert(&step));

    assert_eq!(
        calendar.read::<CalendarEvent>(id),
        Some(CalendarEvent::new("Standup", 600, 630))
    );
    let calendar = edited(&calendar, calendar.reapply(&step));
    assert_eq!(calendar.read::<CalendarEvent>(id).unwrap().title, "Retro");
}
