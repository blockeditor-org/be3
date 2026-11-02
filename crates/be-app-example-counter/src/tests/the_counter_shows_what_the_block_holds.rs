use super::*;

#[test]
fn the_counter_shows_what_the_block_holds() {
    let mut harness = Harness::new();

    harness.set_count(3);
    harness.run();
    assert_eq!(harness.shown(), "\"3\"");

    harness.set_count(2);
    harness.run();
    assert_eq!(harness.shown(), "\"2\"");

    harness.set_count(0);
    harness.run();
    assert_eq!(harness.shown(), "\"0\"");
}
