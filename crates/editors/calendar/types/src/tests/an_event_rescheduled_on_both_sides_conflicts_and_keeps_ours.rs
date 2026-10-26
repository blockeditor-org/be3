use super::*;

#[test]
fn an_event_rescheduled_on_both_sides_conflicts_and_keeps_ours() {
    let (base, id) = scheduled();
    let ours = edited(
        &base,
        [base
            .root()
            .update(id, &CalendarEvent::new("Standup", 600, 615))],
    );
    let theirs = edited(
        &base,
        [base
            .root()
            .update(id, &CalendarEvent::new("Daily", 700, 715))],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 2);
    assert_eq!(
        merged
            .root()
            .events
            .get(id)
            .map(|event| event.value.clone()),
        Some(CalendarEvent::new("Daily", 600, 615))
    );
}
