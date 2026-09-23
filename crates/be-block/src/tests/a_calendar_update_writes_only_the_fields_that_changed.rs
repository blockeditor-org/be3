use super::*;

#[test]
fn a_calendar_update_writes_only_the_fields_that_changed() {
    let (calendar, id) = scheduled();
    let root = calendar.root();

    assert_eq!(
        root.update(id, &CalendarEvent::new("Standup", 540, 555))
            .0
            .len(),
        0
    );
    assert_eq!(
        root.update(id, &CalendarEvent::new("Sync", 540, 600))
            .0
            .len(),
        2
    );
    assert_eq!(
        root.update(crate::ObjectId::new(), &CalendarEvent::new("Sync", 0, 1)),
        crate::Edit::default()
    );
    assert_eq!(CalendarEvent::new("Backwards", 90, 30).ends(), 90);
}
