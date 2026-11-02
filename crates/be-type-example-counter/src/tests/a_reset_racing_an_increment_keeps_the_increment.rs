use super::*;

#[test]
fn a_reset_racing_an_increment_keeps_the_increment() {
    let base = edited(&CounterContent::default(), [Counter::add(5)]);
    let reset = base.root().reset();
    let increment = Counter::add(1);

    assert_eq!(
        edited(&base, [reset.clone(), increment.clone()])
            .root()
            .value(),
        1
    );
    assert_eq!(edited(&base, [increment, reset]).root().value(), 1);

    let ours = edited(&base, [base.root().reset()]);
    let theirs = edited(&base, [Counter::add(2)]);
    let (merged, conflicts) = merged(&base, &ours, &theirs);
    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().value(), 2);
}
