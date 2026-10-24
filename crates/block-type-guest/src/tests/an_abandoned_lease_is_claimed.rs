use be_protocol::SessionState;

use super::*;

#[test]
fn an_abandoned_lease_is_claimed() {
    let mut module = checklist_module();
    let commands = module.handle(Event::Open {
        replica: REPLICA,
        block: BLOCK,
        content_type: ChecklistContent::CONTENT_TYPE,
        organization: ORGANIZATION,
        client: 2,
        tag: TAG,
        head: None,
        session: SessionState {
            generation: 4,
            owner: Some(1),
            participants: vec![2],
            clean_at: None,
        },
    });
    let claims: Vec<_> = commands
        .iter()
        .filter(|command| matches!(command, Command::ClaimLease { .. }))
        .collect();
    assert_eq!(
        claims,
        [&Command::ClaimLease {
            replica: REPLICA,
            generation: 4,
        }]
    );
    let again = module.handle(Event::Lease {
        replica: REPLICA,
        session: SessionState {
            generation: 4,
            owner: Some(1),
            participants: vec![2],
            clean_at: None,
        },
        resynchronize: false,
    });
    assert!(!again
        .iter()
        .any(|command| matches!(command, Command::ClaimLease { .. })));
    let granted = module.handle(Event::Lease {
        replica: REPLICA,
        session: SessionState {
            generation: 5,
            owner: Some(2),
            participants: vec![2],
            clean_at: None,
        },
        resynchronize: false,
    });
    assert!(granted.iter().any(|command| matches!(
        command,
        Command::Status { status, .. } if status.owner
    )));
}
