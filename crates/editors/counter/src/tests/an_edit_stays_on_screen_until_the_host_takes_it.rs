use super::*;

#[test]
fn an_edit_stays_on_screen_until_the_host_takes_it() {
    let mut harness = Harness::new();

    harness.click("counter.increment");
    harness.editor.run();
    harness.publish();
    harness.editor.run();

    assert_eq!(harness.count(), 0);
    assert_eq!(harness.shown(), "\"1\"");

    harness.run();

    assert_eq!(harness.count(), 1);
    assert_eq!(harness.shown(), "\"1\"");
}
