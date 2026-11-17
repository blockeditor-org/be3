use be_block_protocol::{MembershipChange, MembershipEntry, SessionState};

use super::*;

/// The session speaks its first participant's build; a participant of another build does not
/// speak it, nor hold the lease even when it claims it, until the ones before it leave.
#[test]
fn the_first_participant_names_the_build_the_session_speaks() {
    let other = be_block_store::Hash::from_bytes([9; 32]);
    let mut session = SessionState::default();
    session.write(0, 1, joined(true, TAG));
    session.write(0, 2, joined(true, other));
    session.write(0, 3, joined(true, TAG));
    assert_eq!(session.tag(), Some(TAG));
    assert!(session.speaks(1) && session.speaks(3));
    assert!(!session.speaks(2));
    session.write(0, 2, MembershipChange::Claimed);
    assert!(session.is_holder(1), "a claim by another build took the lease");

    session.write(0, 1, MembershipChange::Left);
    assert_eq!(session.tag(), Some(other));
    assert!(session.speaks(2));
    assert!(!session.speaks(3));
    assert!(session.is_holder(2));

    // An entry past a gap is not applied: the participant joins again.
    let index = session.index;
    let skipped = MembershipEntry {
        index: index + 2,
        after: 0,
        client: 3,
        change: MembershipChange::Left,
    };
    assert!(!session.apply(&skipped));
    assert_eq!(session.index, index);
    assert!(session.has(3));
}
