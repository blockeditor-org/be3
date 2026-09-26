use super::*;

#[test]
fn an_edit_stays_on_screen_until_the_host_takes_it() {
    let mut harness = Harness::new();
    let store = harness.editor.store();
    store.defer(true);

    harness.click("counter.increment");
    harness.run();
    harness.editor.hold(None, CounterContent::default());
    harness.run();

    assert_eq!(harness.count(), 0);
    assert_eq!(harness.shown(), "\"1\"");

    store.defer(false);
    harness.run();

    assert_eq!(harness.count(), 1);
    assert_eq!(harness.shown(), "\"1\"");
}
