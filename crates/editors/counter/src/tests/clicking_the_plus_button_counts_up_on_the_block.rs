use super::*;

#[test]
fn clicking_the_plus_button_counts_up_on_the_block() {
    let mut harness = Harness::new();

    harness.click("counter.increment");
    harness.run();
    harness.click("counter.increment");
    harness.run();

    assert_eq!(harness.count(), 2);
    assert_eq!(harness.shown(), "\"2\"");
    harness
        .editor
        .snapshot("clicking_the_plus_button_counts_up_on_the_block");
}
