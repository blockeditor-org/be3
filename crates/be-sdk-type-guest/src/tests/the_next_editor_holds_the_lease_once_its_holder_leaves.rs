use be_block_protocol::{JoinedParticipant, MembershipChange, SessionState};

use super::*;

/// A replica derives the holder from the session's membership: it holds the lease once the
/// holder before it leaves, with no claim of its own.
#[test]
fn the_next_editor_holds_the_lease_once_its_holder_leaves() {
    let module = checklist_module();
    let mut lone = Lone::new(&module);
    let tag = be_block_store::Hash::from_bytes(opening(2, None, SessionState::default()).tag.0);
    let mut session = SessionState::default();
    for client in [1, 2] {
        session.write(
            0,
            client,
            MembershipChange::Joined(JoinedParticipant {
                tag,
                may_edit: true,
            }),
        );
    }
    let replica = Replica::new(
        &mut lone.host,
        Opening {
            content_type: ChecklistContent::CONTENT_TYPE,
            ..opening(2, None, session.clone())
        },
    );
    let (calls, _) = lone.exchange();
    assert!(!calls.iter().any(|call| matches!(call, Call::Claim { .. })));
    assert!(!calls.iter().any(|call| matches!(
        call,
        Call::ReportStatus { status, .. } if status.holder
    )));

    session.write(0, 1, MembershipChange::Left);
    replica.lease(&mut lone.host, session);
    let (calls, _) = lone.exchange();
    assert!(!calls.iter().any(|call| matches!(call, Call::Claim { .. })));
    assert!(calls.iter().any(|call| matches!(
        call,
        Call::ReportStatus { status, .. } if status.holder
    )));
}
