use super::*;

/// The session speaks its first participant's build; a participant of another build does not
/// speak it until the ones before it leave.
#[test]
fn the_first_participant_names_the_build_the_session_speaks() {
    let other = be_block_store::Hash::from_bytes([9; 32]);
    let mut lease = Lease::new();
    lease.join(1, true, TAG, 0);
    lease.join(2, true, other, 0);
    lease.join(3, true, TAG, 0);
    assert_eq!(lease.state().tag, Some(TAG));
    assert!(lease.speaks(1) && lease.speaks(3));
    assert!(!lease.speaks(2));

    lease.leave(1, 100);
    assert_eq!(lease.state().tag, Some(other));
    assert!(lease.speaks(2));
    assert!(!lease.speaks(3));
}
