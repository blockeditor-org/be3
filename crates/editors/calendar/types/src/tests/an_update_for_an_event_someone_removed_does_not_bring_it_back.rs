use super::*;

#[test]
fn an_update_for_an_event_someone_removed_does_not_bring_it_back() {
    let (base, id) = scheduled();
    let update = base
        .root()
        .update(id, &CalendarEvent::new("Retro", 600, 660));

    let sequenced = edited(&base, [Calendar::remove(id), update]);

    assert!(sequenced.root().events.is_empty());
    let removed = edited(&base, [Calendar::remove(id)]);
    assert_eq!(
        removed
            .root()
            .update(id, &CalendarEvent::new("Retro", 600, 660)),
        Edit::default()
    );
}
