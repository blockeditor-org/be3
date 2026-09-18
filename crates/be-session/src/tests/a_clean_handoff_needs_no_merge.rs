use super::*;

#[test]
fn a_clean_handoff_needs_no_merge() {
    let commits = commits();
    let head = write(&commits, "everything the phone typed\n", 1_000, None);

    let mut lease = Lease::new();
    lease.join(1, 0);
    lease.join(2, 0);
    let generation = lease.generation();
    assert!(lease.heartbeat(1, generation, Some(head), 100));

    lease.leave(1, LEASE_MILLISECONDS);
    assert_eq!(lease.owner(), Some(2));
    assert_eq!(lease.clean_at(), Some(head));
    assert!(
        !takeover_needs_merge(lease.clean_at(), Some(head)),
        "a session that was durable at the head still demanded a merge"
    );
    assert_eq!(
        resume(&commits, Some(head), Some(head)).unwrap(),
        Resume::UpToDate
    );

    let unpublished = write(&commits, "typed after the last save\n", 2_000, Some(head));
    assert!(takeover_needs_merge(Some(head), Some(unpublished)));

    assert_eq!(
        resume(&commits, Some(head), Some(unpublished)).unwrap(),
        Resume::FastForward { to: unpublished },
        "the returning owner should catch up rather than merge"
    );
}
