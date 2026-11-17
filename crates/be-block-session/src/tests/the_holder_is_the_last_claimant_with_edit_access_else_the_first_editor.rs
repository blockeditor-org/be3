use be_block_protocol::{MembershipChange, SessionState};

use super::*;

/// With no claim the earliest-joined editor holds the lease; a claim takes it at once, and it
/// goes back to the earliest editor once the claimant leaves or loses edit access. A viewer
/// never holds it.
#[test]
fn the_holder_is_the_last_claimant_with_edit_access_else_the_first_editor() {
    let mut session = SessionState::default();
    session.write(0, 1, joined(false, TAG));
    assert_eq!(session.holder(), None);
    session.write(0, 2, joined(true, TAG));
    session.write(0, 3, joined(true, TAG));
    assert_eq!(session.holder(), Some(2));

    session.write(4, 3, MembershipChange::Claimed);
    assert!(session.is_holder(3));
    session.write(5, 2, MembershipChange::Claimed);
    assert!(session.is_holder(2));

    // The last claimant loses edit access: the claim before it holds again.
    session.write(6, 2, MembershipChange::Access(false));
    assert!(session.is_holder(3));
    // Nothing changes, nothing is written.
    assert!(session.write(6, 2, MembershipChange::Access(false)).is_none());
    assert!(session.write(6, 9, MembershipChange::Left).is_none());

    session.write(7, 3, MembershipChange::Left);
    assert_eq!(session.holder(), None);
    session.write(8, 2, MembershipChange::Access(true));
    assert!(session.is_holder(2));
    assert_eq!(session.clients(), vec![1, 2]);
    assert_eq!(session.index, 8);
}
