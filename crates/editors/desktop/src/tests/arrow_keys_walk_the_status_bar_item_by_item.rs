use super::*;

#[test]
fn arrow_keys_walk_the_status_bar_item_by_item() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.status");
    fixture.settle();
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    let focused_on = |fixture: &Fixture, test_id: &str| {
        let document = fixture.test.document();
        document
            .find_test_id(test_id)
            .is_some_and(|node| document.focus_is_within(node))
    };
    assert!(focused_on(&fixture, "desktop.status"));

    fixture.test.key(Key::ArrowRight, Modifiers::NONE);
    fixture.settle();
    assert!(
        focused_on(&fixture, "desktop.clock"),
        "the arrow moves from the status button to the clock beside it, though both sit in a group of their own"
    );
    fixture.test.key(Key::ArrowLeft, Modifiers::NONE);
    fixture.settle();
    assert!(focused_on(&fixture, "desktop.status"), "and back again");
}
