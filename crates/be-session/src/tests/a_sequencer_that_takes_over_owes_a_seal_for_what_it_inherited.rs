use super::*;

#[test]
fn a_sequencer_that_takes_over_owes_a_seal_for_what_it_inherited() {
    let head = write(&commits(), "base\n", 1_000, None);
    let mut follower = Follower::new(7);
    follower.submit(b"typed while following".to_vec());

    let pending = follower.take_pending();
    let mut owner = Sequencer::continuing(Some(head), 5, 2);

    assert_eq!(follower.pending(), 0);
    assert_eq!(pending.len(), 1);
    assert!(!owner.is_clean());
    assert_eq!(owner.pending_operations(), 2);

    let (id, payload) = pending.into_iter().next().unwrap();
    let op = owner.accept(id, payload).unwrap();
    assert_eq!(op.sequence, 6, "the new owner restarted the numbering");
    assert_eq!(owner.pending_operations(), 3);

    let sealed = write(&commits(), "base\nsealed\n", 2_000, Some(head));
    owner.sealed(sealed);
    assert!(owner.is_clean());
    assert_eq!(owner.sequence(), 6);
}
