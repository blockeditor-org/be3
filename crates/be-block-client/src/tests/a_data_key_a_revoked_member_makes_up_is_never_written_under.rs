use super::*;

use be_block_graph::Access;
use be_block_keys::{CarriedKey, Heads};
use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};

/// A revoked reader still holds the scope epoch from before their revocation. With the server,
/// they make up a data key, wrap it under that epoch (so no seal names them) and have it listed
/// as the block's newest. Naming no head, a head no log holds, one from before the revocation
/// or the newest, an editor's device opens it but never writes under it: it was opened through
/// an epoch the log outdates, so the save gives the scope a new epoch first, also on a device
/// that has read no log yet (signed access 3d).
#[tokio::test]
async fn a_data_key_a_revoked_member_makes_up_is_never_written_under() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let mut head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .grant(team, carol.account(), Access::Edit)
        .await
        .unwrap();
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    assert_eq!(text(&carol, note).await, "before");
    bob.refresh_keys(None).await.unwrap();
    let old = bob.keys().scope_key(team).unwrap();
    let (entries, _) = carol.checked_access(team).await.unwrap();
    let before_revocation = entries.last().unwrap().hash();

    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    carol.refresh_keys(None).await.unwrap();
    let (entries, _) = carol.checked_access(team).await.unwrap();
    let after_revocation = entries.last().unwrap().hash();

    let organization = admin.organization();
    // Each head the made-up key names, and whether a device made after it is planted saves:
    // one that has read no log yet. Naming the head after the revocation, only that it was
    // opened through the outdated epoch gives it away.
    for (round, (named, fresh)) in [
        (None, false),
        (Some([9; 32]), false),
        (Some(before_revocation), false),
        (Some(after_revocation), false),
        (Some(after_revocation), true),
        (None, true),
    ]
    .into_iter()
    .enumerate()
    {
        let newest = carol
            .keys()
            .keys_of(note, KeyKind::Data)
            .last()
            .unwrap()
            .epoch;
        let made_up = Uuid::new_v4();
        let carried = CarriedKey {
            key: [round as u8 + 1; 32],
            block: note.into_bytes(),
            root: None,
            heads: Heads {
                block: named,
                organization: None,
            },
        };
        let sealed = bob.keys().ring().seal(old, &carried.encode()).unwrap();
        let database = harness.database();
        database
            .execute(
                "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
                 VALUES (?1, ?2, ?3, 'data', ?4, 0)",
                rusqlite::params![
                    organization.to_string(),
                    made_up.to_string(),
                    note.to_string(),
                    i64::try_from(newest + 10).unwrap()
                ],
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    organization.to_string(),
                    made_up.to_string(),
                    old.to_string(),
                    bob.account().to_string(),
                    sealed
                ],
            )
            .unwrap();
        drop(database);
        let second;
        let device = match fresh {
            true => {
                second = harness.second(&carol).await;
                &second
            }
            false => {
                carol.refresh_keys(None).await.unwrap();
                &carol
            }
        };
        assert!(
            device.keys().trusted(made_up),
            "the made-up key did not open, so the test shows nothing"
        );

        let epochs = device.keys().keys_of(team, KeyKind::Scope).len();
        let text_now = format!("after {round}");
        head = device
            .save(note, &TextBlock::of(&text_now), head)
            .await
            .unwrap()
            .published();
        assert_eq!(
            device.keys().keys_of(team, KeyKind::Scope).len(),
            epochs + 1,
            "the save wrote under a key opened through an outdated epoch (round {round})"
        );
        let written = device
            .keys()
            .keys_of(note, KeyKind::Data)
            .last()
            .unwrap()
            .id;
        carol.refresh_keys(None).await.unwrap();
        assert_ne!(written, made_up);
        bob.refresh_keys(None).await.unwrap();
        assert!(!bob.keys().has(written));
    }

    harness.stop().await;
}
