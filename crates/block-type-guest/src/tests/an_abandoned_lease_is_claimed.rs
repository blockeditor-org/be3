use be_protocol::SessionState;

use super::*;

fn abandoned(generation: u64, owner: ClientId) -> SessionState {
    SessionState {
        generation,
        owner: Some(owner),
        participants: vec![2],
        clean_at: None,
    }
}

#[test]
fn an_abandoned_lease_is_claimed() {
    let module = checklist_module();
    let mut lone = Lone::new(&module);
    let replica = Replica::new(
        &mut lone.host,
        Opening {
            content_type: ChecklistContent::CONTENT_TYPE,
            ..opening(2, None, abandoned(4, 1))
        },
    );
    let (calls, _) = lone.exchange();
    let claims: Vec<_> = calls
        .iter()
        .filter(|call| matches!(call, Call::ClaimLease { .. }))
        .collect();
    assert!(matches!(
        claims.as_slice(),
        [Call::ClaimLease { generation: 4 }]
    ));
    replica.lease(&mut lone.host, abandoned(4, 1), false);
    let (again, _) = lone.exchange();
    assert!(
        !again
            .iter()
            .any(|call| matches!(call, Call::ClaimLease { .. }))
    );
    replica.lease(&mut lone.host, abandoned(5, 2), false);
    let (granted, _) = lone.exchange();
    assert!(granted.iter().any(|call| matches!(
        call,
        Call::ReportStatus { status } if status.owner
    )));
}
