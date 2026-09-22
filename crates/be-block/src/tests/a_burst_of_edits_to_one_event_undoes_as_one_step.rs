use super::*;

#[test]
fn a_burst_of_edits_to_one_event_undoes_as_one_step() {
    let event = meeting();
    let other = CalendarEvent::new("Lunch".to_owned(), 720, 780);
    let mut calendar = scheduled(&event);
    calendar.apply(&CalendarOp::AddEvent {
        event: other.clone(),
    });
    let retitle = |calendar: &CalendarContent, id: Uuid, title: &str| CalendarOp::UpdateEvent {
        event: CalendarEvent {
            title: title.to_owned(),
            ..calendar.event(id).unwrap().clone()
        },
    };

    let first = retitle(&calendar, event.id, "S");
    let mut step = calendar.step(&first).unwrap();
    calendar.apply(&first);
    let second = retitle(&calendar, event.id, "Sync");
    let next = calendar.step(&second).unwrap();
    calendar.apply(&second);
    assert!(CalendarContent::absorb(&mut step, next).is_ok());

    let elsewhere = retitle(&calendar, other.id, "Brunch");
    let unrelated = calendar.step(&elsewhere).unwrap();
    assert!(matches!(
        CalendarContent::absorb(&mut step, unrelated),
        Err(CalendarStep::Changed { .. })
    ));

    for operation in calendar.revert(&step) {
        calendar.apply(&operation);
    }
    assert_eq!(calendar.event(event.id).unwrap().title, "Standup");
}
