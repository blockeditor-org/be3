use super::*;

#[test]
fn only_a_participant_that_may_edit_is_granted_the_lease() {
    let mut lease = Lease::new();
    lease.join(1, false, 0);
    assert_eq!(lease.owner(), None);
    assert_eq!(lease.state().participants, vec![1]);

    lease.join(2, true, 0);
    lease.join(3, false, 0);
    lease.join(4, true, 0);
    assert_eq!(lease.owner(), Some(2));

    lease.leave(2, 100);
    assert_eq!(lease.owner(), Some(4));

    lease.leave(4, 200);
    assert_eq!(lease.owner(), None);
    assert_eq!(lease.state().participants, vec![1, 3]);
}
