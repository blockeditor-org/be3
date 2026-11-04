use super::*;

#[test]
fn the_clock_counts_only_notifications_not_yet_seen_in_its_list() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.notify(&[
        notification(1, "Mail", "Mia Chen", "Lunch at noon?"),
        notification(2, "Mail", "Sam", "See you there"),
    ]);
    assert_eq!(fixture.test.label("desktop.clock.unread"), "2");

    fixture.test.click("desktop.clock");
    fixture.settle();
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.clock.unread"),
        "opening the list reads what it shows"
    );
    assert!(fixture.says("Mia Chen"), "and keeps it listed");

    fixture.notify(&[notification(3, "Chat", "Ana", "Running late")]);
    assert_eq!(
        fixture.test.label("desktop.clock.unread"),
        "1",
        "only what came after is new"
    );
    fixture.notify(&[notification(1, "Mail", "Mia Chen", "Lunch at one instead")]);
    assert_eq!(
        fixture.test.label("desktop.clock.unread"),
        "2",
        "a notification replaced in place is new again"
    );
}
