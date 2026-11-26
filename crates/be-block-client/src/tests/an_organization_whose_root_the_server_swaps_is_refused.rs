use super::*;

use be_block_keys::Entry;

/// A company organization's access log is rooted at whoever made it: their device pins the root
/// from making it, and a member's from the invitation, sealed with its code. A server that roots
/// the organization at someone else is refused by both, and the personal organization has no
/// log at all.
#[tokio::test]
async fn an_organization_whose_root_the_server_swaps_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    let organization = owner.organization();

    // A member below Edit is served a proof path: the same entries, some reduced.
    let hashes = |log: Vec<be_block_keys::Step>| -> Vec<be_block_keys::EntryHash> {
        log.iter().map(be_block_keys::Step::hash).collect()
    };
    let made = hashes(owner.organization_log().await.unwrap());
    assert_eq!(
        made.len(),
        5,
        "an organization's log is its root, then each invitation and the member it signed in"
    );
    assert_eq!(
        hashes(member.organization_log().await.unwrap()),
        made,
        "a member pins the root from the invitation"
    );
    assert_eq!(
        hashes(
            harness
                .second(&member)
                .await
                .organization_log()
                .await
                .unwrap()
        ),
        made,
        "and keeps it on every device, sealed to itself"
    );

    let forged = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        be_block_keys::LogKind::Organization,
        organization.into_bytes(),
    );
    harness
        .database()
        .execute(
            "UPDATE organization_log SET entry = ?1 WHERE organization_id = ?2 AND position = 0",
            rusqlite::params![forged.encode(), organization.to_string()],
        )
        .unwrap();
    for (who, device) in [("its creator", &owner), ("a member", &member)] {
        assert!(
            matches!(
                device.organization_log().await,
                Err(ClientError::Forged(log)) if log == organization
            ),
            "{who}'s device took an organization root the server swapped"
        );
    }

    harness.stop().await;
}
