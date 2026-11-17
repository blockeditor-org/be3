use super::*;

use be_block_graph::Access;
use be_block_keys::Level;

/// Whether `block`'s log, as `peer` checks it, revokes `account`, its last word on them.
async fn revoked(peer: &Peer<MemoryStore>, block: Uuid, account: Uuid) -> bool {
    let (_, checked) = peer.checked_access(block).await.unwrap();
    checked
        .grants
        .get(&account.into_bytes())
        .is_some_and(|(level, _)| *level == Level::None)
}

/// A removal made from a device carries its signed revocations, so the log revokes at once; one
/// the server makes on its own (a block's agent whose maker no longer manages its block) is
/// listed to those who manage the block, not to anyone else, sealed to no one meanwhile, and
/// signed into the log by a manager's next `tend` (signed access 3c).
#[tokio::test]
async fn a_revocation_the_server_makes_is_signed_into_the_log_by_a_manager() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let leaver = harness.member(&admin, "leaver@example.com").await;
    let team = workspace(&admin, "Team").await;
    for (account, access) in [
        (member.account(), Access::Manage),
        (leaver.account(), Access::Edit),
    ] {
        let identity = admin.verified_identity(account).unwrap();
        admin.share(team, account, access, &identity).await.unwrap();
    }

    // Removed from a device: revoked in the log by the call itself.
    admin.remove_member(leaver.account()).await.unwrap();
    assert!(revoked(&admin, team, leaver.account()).await);
    assert!(
        admin
            .key_work()
            .await
            .unwrap()
            .unsigned_revocations
            .is_empty()
    );

    // The member makes a block's agent of the workspace, then stops managing it: the server
    // deletes the agent on its own, revoking its grant.
    member.refresh_keys(None).await.unwrap();
    let secret = be_block_keys::RecoveryPhrase::generate().secret();
    let agent = Uuid::from_bytes(be_block_keys::agent_account(
        member.account().into_bytes(),
        &secret.identity(),
    ));
    let bound = be_block_protocol::BoundAgent {
        block: team,
        signed: member
            .grant_record(team, agent, Access::Edit, secret.identity())
            .await
            .unwrap(),
    };
    crate::ask(
        be_block_protocol::write::<be_block_protocol::CreatedAgent, _>(
            &member.connection(),
            Some(member.organization()),
            be_block_protocol::Op::CreateAgent {
                account: agent,
                name: "Helper".into(),
                recovery: secret.public().0,
                signing: secret.signing().0,
                contact: Vec::new(),
                escrow: Vec::new(),
                bound_to: Some(bound),
                member: member.agent_record(agent, secret.identity()).await.unwrap(),
            },
        ),
    )
    .await
    .unwrap();
    admin
        .grant(team, member.account(), Access::View)
        .await
        .unwrap();

    let work = admin.key_work().await.unwrap();
    assert!(
        work.unsigned_revocations
            .iter()
            .any(|revocation| revocation.block == team && revocation.account == agent),
        "the revocation the server made waits for a manager: {:?}",
        work.unsigned_revocations
    );
    assert!(
        !work
            .unsealed
            .iter()
            .any(|unsealed| unsealed.account == agent),
        "nothing is due to the agent while its revocation waits"
    );
    assert!(
        member
            .key_work()
            .await
            .unwrap()
            .unsigned_revocations
            .is_empty(),
        "one who no longer manages the block is not asked to sign"
    );
    assert!(!revoked(&admin, team, agent).await);

    admin.tend().await.unwrap();
    assert!(revoked(&admin, team, agent).await, "the manager signed it");
    assert!(
        admin
            .key_work()
            .await
            .unwrap()
            .unsigned_revocations
            .is_empty()
    );

    harness.stop().await;
}
