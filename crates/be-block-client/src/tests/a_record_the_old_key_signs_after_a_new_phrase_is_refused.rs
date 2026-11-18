use super::*;

use be_block_graph::Access;
use be_block_keys::Entry;
use be_block_protocol::RecoveryKey;

/// Taking a new recovery phrase retires the old keys in every access log the account signed in,
/// endorsed by the old key and signed by the new: the server and every device take the logs
/// after it as before, and refuse anything the old key signs after it, so whoever holds the old
/// phrase cannot act as the account there.
#[tokio::test]
async fn a_record_the_old_key_signs_after_a_new_phrase_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let organization = owner.organization();
    let team = workspace(&owner, "Team").await;
    let public = owner.verified_identity(member.account()).unwrap();
    owner
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();
    let old = owner.keys().identity().clone();

    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let RecoveryKey { logs, .. } = crate::ask(be_block_protocol::read::<
        be_block_protocol::RecoveryKey,
        _,
    >(
        &owner.connection(),
        None,
        be_block_protocol::Query::RecoveryKey,
    ))
    .await
    .unwrap();
    assert!(
        logs.iter().any(|log| log.log == team) && logs.iter().any(|log| log.log == organization),
        "the logs the owner signed in are its workspace's and its organization's: {logs:?}"
    );
    take_phrase(&owner.connection(), owner.account(), &old, &fresh).await;

    let laptop = harness.second(&member).await;
    let retired = laptop.access_log(team).await.unwrap();
    assert_eq!(
        retired.len(),
        4,
        "the workspace's log is its root, its placement, the grant sharing it and the retirement"
    );
    let organization_log = laptop.organization_log().await.unwrap();
    assert_eq!(
        organization_log.len(),
        4,
        "the root, the invitation, the member it signed in, the retirement"
    );

    // Whoever holds the old phrase retires the account again, to keys of their own.
    let thief = be_block_keys::RecoveryPhrase::generate().secret();
    let database = harness.database();
    let again = Entry::retirement(
        &old,
        &thief,
        owner.account().into_bytes(),
        team.into_bytes(),
        retired[3].hash(),
    );
    database
        .execute(
            "INSERT INTO access_log (organization_id, block_id, position, entry)
             VALUES (?1, ?2, 4, ?3)",
            rusqlite::params![organization.to_string(), team.to_string(), again.encode()],
        )
        .unwrap();
    let head = organization_log[3].hash();
    let again = Entry::retirement(
        &old,
        &thief,
        owner.account().into_bytes(),
        organization.into_bytes(),
        head,
    );
    database
        .execute(
            "INSERT INTO organization_log (organization_id, position, entry)
             VALUES (?1, 4, ?2)",
            rusqlite::params![organization.to_string(), again.encode()],
        )
        .unwrap();
    drop(database);

    assert!(
        matches!(laptop.access_log(team).await, Err(ClientError::Forged(block)) if block == team),
        "a device took a record the old key signed after its retirement"
    );
    assert!(
        matches!(laptop.organization_log().await, Err(ClientError::Forged(log)) if log == organization),
        "a device took an organization record the old key signed after its retirement"
    );

    harness.stop().await;
}
