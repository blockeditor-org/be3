use super::*;

#[test]
fn calendars_merge_each_event_field_by_field() {
    let (base, id) = scheduled();
    let ours = edited(
        &base,
        [base
            .root()
            .update(id, &CalendarEvent::new("Retro", 540, 555))],
    );
    let (lunch, add_lunch) = Calendar::add(&CalendarEvent::new("Lunch", 720, 780));
    let theirs = edited(
        &base,
        [
            base.root()
                .update(id, &CalendarEvent::new("Standup", 600, 630)),
            add_lunch,
        ],
    );

    let MergeResult::Clean(merged) = CalendarContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different fields");
    };

    assert_eq!(
        merged.read::<CalendarEvent>(id),
        Some(CalendarEvent::new("Retro", 600, 630))
    );
    assert!(merged.read::<CalendarEvent>(lunch).is_some());
}
