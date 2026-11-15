use super::*;

/// A seal through a sequence cuts the order there, unless an operation after it was based
/// earlier: the ones between stay, since a replica opening on the seal rebases it over them.
#[test]
fn a_seal_trims_the_order_but_keeps_what_later_operations_rebase_onto() {
    let mut order = Sequencer::starting(0);
    for counter in 1..=6 {
        order.number(
            Submission {
                id: OpId { origin: 1, counter },
                base: counter - 1,
                barrier: false,
                payload: Vec::new(),
            },
            None,
        );
    }
    // Sealed through 5, with 6 based on 3: 4 and 5 stay.
    assert_eq!(order.sealed(5, [3]), 3);
    assert_eq!(order.floor(), 3);
    // A seal never moves the cut back, nor past what it holds.
    assert_eq!(order.sealed(2, []), 3);
    assert_eq!(order.sealed(9, []), 6);
}
