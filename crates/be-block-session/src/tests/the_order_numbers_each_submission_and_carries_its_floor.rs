use super::*;

fn submission(counter: u64, base: u64, barrier: bool) -> Submission {
    Submission {
        id: OpId { origin: 7, counter },
        base,
        barrier,
        payload: vec![counter as u8],
    }
}

/// Each submission takes the next number and the floor of the order as it stands: none until a
/// barrier is numbered, then the last barrier.
#[test]
fn the_order_numbers_each_submission_and_carries_its_floor() {
    let mut order = Sequencer::starting(4);
    let first = order.number(submission(1, 4, false), Some(10));
    assert_eq!((first.sequence, first.base, first.floor), (5, 4, 0));
    assert_eq!(first.received, Some(10));

    let barrier = order.number(submission(2, 5, true), None);
    assert_eq!((barrier.sequence, barrier.floor), (6, 6));
    let after = order.number(submission(3, 5, false), None);
    assert_eq!(
        (after.sequence, after.floor),
        (7, 6),
        "based before the barrier, it is passed over"
    );
}
