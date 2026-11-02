use super::*;

#[test]
fn undoing_a_reset_restores_only_what_was_reset() {
    let mut content = edited(&CounterContent::default(), [Counter::add(5)]);

    let reset = content.root().reset();
    let step = undone(&mut content, reset);
    content.apply(&Counter::add(2));
    reverted(&mut content, &step);

    assert_eq!(content.root().value(), 7);
}
