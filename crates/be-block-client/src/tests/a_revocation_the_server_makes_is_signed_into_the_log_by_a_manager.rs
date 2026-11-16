use super::*;

use be_block_graph::Access;
use be_block_keys::{Level, Record};

/// Removing a member revokes their grants on the server at once; the block's log, which names
/// the grant, is told by a device that manages the block, signing the revocation in its next
/// `tend`, so no device seals to them by the log meanwhile or after (signed access 3c).
#[tokio::test]
async fn a_revocation_the_server_makes_is_signed_into_the_log_by_a_manager() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let team = workspace(&admin, "Team").await;
    let identity = admin.verified_identity(member.account()).unwrap();
    admin
        .share(team, member.account(), Access::Edit, &identity)
        .await
        .unwrap();

    admin.remove_member(member.account()).await.unwrap();
    let work = admin.key_work().await.unwrap();
    assert!(
        work.unsigned_revocations
            .iter()
            .any(|revoked| revoked.block == team && revoked.account == member.account()),
        "the revocation the server made waits to be signed: {:?}",
        work.unsigned_revocations
    );
    admin.tend().await.unwrap();
    let (log, checked) = admin.checked_access(team).await.unwrap();
    assert!(
        matches!(&log.last().unwrap().record, Record::Grant(grant)
            if grant.grantee == member.account().into_bytes() && grant.level == Level::None),
        "the log ends with the revocation, signed"
    );
    assert_eq!(
        checked.grants.get(&member.account().into_bytes()).map(|(level, _)| *level),
        Some(Level::None)
    );
    assert!(admin.key_work().await.unwrap().unsigned_revocations.is_empty());

    harness.stop().await;
}
