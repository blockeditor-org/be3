use super::*;

use be_block_graph::Access;
use be_block_keys::{Authority, Entry, Grant, Level, Record};
use be_block_protocol::RecoveryKey;

/// A new recovery phrase retires the old keys in every log that names them: one the account is
/// only granted in, never having signed there, too. Otherwise whoever holds the old phrase could
/// still grant with it where the account manages, and the account's own new key would be
/// refused there.
#[tokio::test]
async fn a_grantee_who_never_signed_retires_its_old_key_too() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let organization = owner.organization();
    let team = workspace(&owner, "Team").await;
    let identity = owner.verified_identity(member.account()).unwrap();
    owner
        .share(team, member.account(), Access::Manage, &identity)
        .await
        .unwrap();
    let old = member.keys().identity().clone();

    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let RecoveryKey { logs, .. } = crate::ask(be_block_protocol::read::<
        be_block_protocol::RecoveryKey,
        _,
    >(
        &member.connection(),
        None,
        be_block_protocol::Query::RecoveryKey,
    ))
    .await
    .unwrap();
    assert!(
        logs.iter().any(|log| log.log == team),
        "the member is granted in the workspace's log, so retires its key there: {logs:?}"
    );
    take_phrase(&member.connection(), member.account(), &old, &fresh).await;

    let laptop = harness.second(&owner).await;
    let retired = laptop.access_log(team).await.unwrap();
    let thief = be_block_keys::RecoveryPhrase::generate().secret();
    let stolen = Entry::sign(
        &old,
        member.account().into_bytes(),
        team.into_bytes(),
        retired.last().map(Entry::hash),
        false,
        Record::Grant(Grant {
            grantee: [9; 16],
            identity: thief.identity(),
            level: Level::Manage,
            authority: Authority::Log,
        }),
    );
    harness
        .database()
        .execute(
            "INSERT INTO access_log (organization_id, block_id, position, entry)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                organization.to_string(),
                team.to_string(),
                retired.len(),
                stolen.encode()
            ],
        )
        .unwrap();
    assert!(
        matches!(laptop.access_log(team).await, Err(ClientError::Forged(block)) if block == team),
        "a device took a grant the old key signed where its account was only granted"
    );

    harness.stop().await;
}
