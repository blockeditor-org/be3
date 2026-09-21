use super::*;

#[test]
fn a_counter_reset_wins_over_the_adds_before_it() {
    let start = CounterContent::new(3);

    let after = counted(
        start,
        &[
            CounterOp::Add { by: 5 },
            CounterOp::Reset,
            CounterOp::Add { by: 2 },
        ],
    );

    assert_eq!(after.count(), 2);
    assert_eq!(
        CounterContent::rebase(CounterOp::Add { by: 2 }, &[CounterOp::Reset]),
        Some(CounterOp::Add { by: 2 })
    );
}
