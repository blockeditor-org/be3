use super::*;
use be_protocol::SessionState;

fn abandoned(generation: u64, owner: ClientId) -> SessionState {
    SessionState {
        generation,
        owner: Some(owner),
        participants: vec![2],
        clean_at: None,
        tag: Some(TAG.0),
    }
}

fn claims(commands: &[Command]) -> Vec<&Command> {
    commands
        .iter()
        .filter(|command| matches!(command, Command::ClaimLease { .. }))
        .collect()
}

#[test]
fn an_abandoned_lease_is_claimed() {
    let mut module = checklist_module();
    let opened = module.handle(Event::Open {
        replica: REPLICA,
        block: BLOCK,
        content_type: ChecklistContent::CONTENT_TYPE,
        organization: ORGANIZATION,
        client: 2,
        tag: TAG,
        head: None,
        session: abandoned(4, 1),
    });
    assert!(
        claims(&opened).is_empty(),
        "a replica claimed before it caught up"
    );
    let token = opened
        .iter()
        .find_map(|command| match command {
            Command::Catchup { token, .. } => Some(*token),
            _ => None,
        })
        .expect("an opened replica catches up from the server");
    let caught = module.handle(Event::Caught {
        replica: REPLICA,
        token,
        result: Ok(Default::default()),
    });
    assert_eq!(
        claims(&caught),
        [&Command::ClaimLease {
            replica: REPLICA,
            generation: 4,
        }]
    );
    let again = module.handle(Event::Lease {
        replica: REPLICA,
        client: 2,
        session: abandoned(4, 1),
        resynchronize: false,
    });
    assert!(claims(&again).is_empty());
    let granted = module.handle(Event::Lease {
        replica: REPLICA,
        client: 2,
        session: abandoned(5, 2),
        resynchronize: false,
    });
    assert!(granted.iter().any(|command| matches!(
        command,
        Command::Status { status, .. } if status.owner
    )));
}
