use super::*;

#[test]
fn calendars_merge_each_event_field_by_field() {
    let event = meeting();
    let base = scheduled(&event);
    let mut ours = base.clone();
    ours.apply(&CalendarOp::UpdateEvent {
        event: CalendarEvent {
            title: "Retro".to_owned(),
            ..event.clone()
        },
    });
    let mut theirs = base.clone();
    let added = CalendarEvent::new("Lunch".to_owned(), 720, 780);
    theirs.apply(&CalendarOp::UpdateEvent {
        event: CalendarEvent {
            start: 600,
            end: 630,
            ..event.clone()
        },
    });
    theirs.apply(&CalendarOp::AddEvent {
        event: added.clone(),
    });

    let MergeResult::Clean(merged) = CalendarContent::merge3(&base, &ours, &theirs) else {
        panic!("the two sides changed different fields");
    };
    let shown = merged.event(event.id).unwrap();
    assert_eq!(
        (shown.title.as_str(), shown.start, shown.end),
        ("Retro", 600, 630)
    );
    assert!(merged.event(added.id).is_some());
    assert_eq!(
        CalendarContent::decode(&merged.encode()),
        Ok(merged.clone())
    );
}
