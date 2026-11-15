use super::*;

use be_block_graph::Access;
use be_block_keys::Entry;
use be_type_text::TextContent;

/// A device takes a block's access log only when its root is the one the device pinned: from
/// making the block, or carried by a trusted key of it, so a contact's device pins it from the
/// share. The server, which cannot write either, cannot root the block anywhere else, nor pass
/// one block's key off as another's.
#[tokio::test]
async fn a_block_whose_root_the_server_swaps_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let member = harness.member(&owner, "member@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    let organization = owner.organization();
    let team = workspace(&owner, "Team").await;
    let public = owner.keys().contact(member.account()).unwrap();
    owner
        .share(team, member.account(), Access::Edit, &public)
        .await
        .unwrap();

    let made = owner.access_log(team).await.unwrap();
    assert_eq!(made.len(), 1, "a new block's log is its root");
    let reopened = harness.second(&member).await;
    assert_eq!(
        reopened.access_log(team).await.unwrap(),
        made,
        "a contact's device takes the root the share's key carried"
    );
    let note = reopened
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    owner.refresh_keys(None).await.unwrap();
    assert!(
        owner.access_log(note).await.is_ok(),
        "a block made in a shared scope is pinned through its key, wrapped under the scope's"
    );

    // The server passes the note's key off as a key of the workspace.
    let data = reopened.current_key(note).unwrap();
    let relabelled = Uuid::new_v4();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'data', 9, 0)",
            rusqlite::params![organization.to_string(), relabelled.to_string(), team.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             SELECT organization_id, ?1, wrapped_by, sender, sealed FROM wrapped_keys
             WHERE key_id = ?2",
            rusqlite::params![relabelled.to_string(), data.to_string()],
        )
        .unwrap();

    // And roots the workspace at someone it controls.
    let forged = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        team.into_bytes(),
    );
    database
        .execute(
            "UPDATE access_log SET entry = ?1 WHERE block_id = ?2 AND position = 0",
            rusqlite::params![forged.encode(), team.to_string()],
        )
        .unwrap();
    drop(database);

    let again = harness.second(&member).await;
    assert!(
        !again.keys().has(relabelled),
        "a key that carries another block was taken as the workspace's"
    );
    for (who, device) in [("its creator", &owner), ("a contact", &again)] {
        assert!(
            matches!(device.access_log(team).await, Err(ClientError::Forged(block)) if block == team),
            "{who}'s device took a root the server swapped"
        );
    }
    assert!(
        again.access_log(note).await.is_ok(),
        "the other block's log still checks"
    );

    // A verified contact seals the member a key of the note carrying another root: one of the
    // two was forged, so the note has no root at all.
    let other = Uuid::new_v4();
    let carried = be_block_keys::CarriedKey {
        key: [4; 32],
        block: note.into_bytes(),
        root: Some(forged.hash()),
    };
    let sealed = owner.keys().identity().seal_for(
        owner.account().into_bytes(),
        &be_block_keys::Party {
            account: member.account().into_bytes(),
            public: member.keys().identity().public(),
        },
        other.as_bytes(),
        &carried.encode(),
    );
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'data', 8, 0)",
            rusqlite::params![organization.to_string(), other.to_string(), note.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                other.to_string(),
                format!("account:{}", member.account()),
                owner.account().to_string(),
                sealed
            ],
        )
        .unwrap();
    drop(database);
    let conflicted = harness.second(&member).await;
    assert_eq!(
        conflicted.keys().root_pin(note),
        crate::keys::RootPin::Conflicting
    );
    assert!(
        matches!(conflicted.access_log(note).await, Err(ClientError::Forged(block)) if block == note),
        "a block whose keys carried two roots is not locked but forged"
    );

    harness.stop().await;
}
