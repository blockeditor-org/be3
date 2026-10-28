use super::*;

#[test]
fn resetting_puts_the_block_back_to_zero() {
    let mut harness = Harness::new();

    harness.click("counter.increment");
    harness.click("counter.decrement");
    harness.click("counter.decrement");
    assert_eq!(harness.count(), -1);

    harness.click("counter.reset");

    assert_eq!(harness.count(), 0);
    assert_eq!(harness.shown(), "\"0\"");
}
