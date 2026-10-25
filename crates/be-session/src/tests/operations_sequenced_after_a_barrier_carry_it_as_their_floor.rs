use super::*;

#[test]
fn operations_sequenced_after_a_barrier_carry_it_as_their_floor() {
    let mut server = Sequencer::new(None);
    server.submit(id(1, 1), 0, false, vec![]).unwrap();
    let barrier = server.submit(id(2, 1), 1, true, vec![]).unwrap();
    assert_eq!(barrier.floor, 0, "a barrier is not below itself");
    let stale = server.submit(id(1, 2), 1, false, vec![]).unwrap();
    assert_eq!(stale.floor, 2);
    assert!(
        stale.base < stale.floor,
        "every replica drops an edit made before the barrier"
    );
    let fresh = server.submit(id(1, 3), 2, false, vec![]).unwrap();
    assert!(fresh.base >= fresh.floor);
}
