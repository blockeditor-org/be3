use super::*;

#[test]
fn an_expired_lease_can_be_claimed_but_a_held_one_cannot() {
    let mut lease = Lease::new();
    lease.join(1, 0);
    assert_eq!(lease.owner(), Some(1));
    let generation = lease.generation();

    lease.join(2, 100);
    assert_eq!(lease.owner(), Some(1));
    assert_eq!(lease.generation(), generation);

    assert_eq!(lease.claim(2, generation, 200), Claim::HeldByAnother);
    assert_eq!(lease.owner(), Some(1));

    assert!(lease.heartbeat(1, generation, None, 1_000));
    assert!(!lease.heartbeat(2, generation, None, 1_000));
    assert_eq!(
        lease.claim(2, generation, 1_000 + LEASE_MILLISECONDS - 1),
        Claim::HeldByAnother
    );

    let taken = lease.claim(2, generation, 1_000 + LEASE_MILLISECONDS);
    assert_eq!(
        taken,
        Claim::Granted {
            generation: generation + 1
        }
    );
    assert_eq!(lease.owner(), Some(2));

    assert_eq!(lease.claim(1, generation, 2_000_000), Claim::Stale);
    assert_eq!(lease.owner(), Some(2));
}
