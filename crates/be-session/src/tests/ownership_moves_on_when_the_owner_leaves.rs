use super::*;

#[test]
fn ownership_moves_on_when_the_owner_leaves() {
    let mut lease = Lease::new();
    lease.join(1, 0);
    lease.join(2, 0);
    lease.join(3, 0);
    assert_eq!(lease.owner(), Some(1));
    assert_eq!(lease.state().participants, vec![1, 2, 3]);

    lease.leave(2, 100);
    assert_eq!(lease.owner(), Some(1));
    assert_eq!(lease.state().participants, vec![1, 3]);

    let generation = lease.generation();
    lease.leave(1, 200);
    assert_eq!(lease.owner(), Some(3));
    assert!(lease.generation() > generation);

    lease.leave(3, 300);
    assert_eq!(lease.owner(), None);
    assert!(lease.is_empty());

    lease.join(4, 400);
    assert_eq!(lease.owner(), Some(4));
}
