use super::*;

#[test]
fn one_deadline_holds_for_every_settle_of_a_host_frame() {
    let mut clock = FrameClock::default();
    let first = clock.deadline(1, true);
    assert_eq!(
        clock.deadline(1, false),
        first,
        "a host frame that lays out again settles against the same deadline"
    );
    assert!(
        clock.deadline(2, false) < first,
        "a stopped clock's budget is longer than a refresh"
    );
}
