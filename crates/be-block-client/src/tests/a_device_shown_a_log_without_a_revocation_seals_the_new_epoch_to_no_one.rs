use super::*;

use be_block_graph::Access;
use be_block_protocol::Unsealed;

/// Each new key epoch names the head of the block's log it was made under, and a device seals
/// it only once it holds the log that far (signed access 3d). The server withholds a
/// revocation from an editor's device and lists the revoked member as due the epoch the
/// rotation after it made: the device seals it to no one, not even a grantee it is due to,
/// until the server serves the log up to the epoch's head; then it seals it to the grantee
/// alone.
#[tokio::test]
async fn a_device_shown_a_log_without_a_revocation_seals_the_new_epoch_to_no_one() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let dave = harness.member(&admin, "dave@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    // So the server would take Carol's seal to Bob, and only her device holds it back.
    befriend(&carol, &bob).await;
    let team = workspace(&admin, "Team").await;
    for (member, access) in [
        (&carol, Access::Edit),
        (&dave, Access::View),
        (&bob, Access::View),
    ] {
        admin.grant(team, member.account(), access).await.unwrap();
    }
    admin.tend().await.unwrap();

    // The revocation, and the rotation it calls for.
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    carol.refresh_keys(None).await.unwrap();
    let epoch = carol.keys().scope_key(team).unwrap();
    let organization = admin.organization();
    let withheld = withhold_newest(&harness, organization, Some(team));
    assert_eq!(
        carol.keys().heads(epoch).block,
        Some(withheld.hash()),
        "the new epoch names a head other than the revocation's"
    );
    // As though no device had sealed it to Dave yet.
    harness
        .database()
        .execute(
            "DELETE FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [epoch.to_string(), format!("account:{}", dave.account())],
        )
        .unwrap();
    // Another device of Carol's, which never held the revocation (one that did refuses a log
    // short of it outright).
    let device = harness.second(&carol).await;

    let work = || async {
        let mut work = device.key_work().await.unwrap();
        work.rotations.clear();
        work.unsealed.retain(|unsealed| unsealed.key != epoch);
        for account in [dave.account(), bob.account()] {
            work.unsealed.push(Unsealed {
                key: epoch,
                account,
            });
        }
        work
    };
    let (_, shown) = device.checked_access(team).await.unwrap();
    assert!(
        shown
            .grants
            .get(&bob.account().into_bytes())
            .is_some_and(|(level, _)| *level == be_block_keys::Level::View),
        "the server withheld no revocation"
    );
    device.tend_with(work().await).await.unwrap();
    assert_eq!(
        sealed_to(&harness, epoch, bob.account()),
        0,
        "the revoked member was sealed the epoch made after their revocation"
    );
    assert_eq!(
        sealed_to(&harness, epoch, dave.account()),
        0,
        "the epoch was sealed by a log short of its head"
    );

    withheld.serve(&harness);
    device.tend_with(work().await).await.unwrap();
    assert_eq!(sealed_to(&harness, epoch, dave.account()), 1);
    assert_eq!(sealed_to(&harness, epoch, bob.account()), 0);

    harness.stop().await;
}
