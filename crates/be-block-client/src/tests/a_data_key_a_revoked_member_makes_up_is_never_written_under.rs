use super::*;

use be_block_graph::Access;
use be_block_keys::{CarriedKey, Heads};
use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};

/// A revoked reader still holds the scope epoch from before their revocation. With the server,
/// they make up a data key, wrap it under that epoch (so no seal names them) and have it listed
/// as the block's newest. Naming no head, a head no log holds, or one from before the
/// revocation, an editor's device opens it but never writes under it: it was opened through an
/// epoch the log outdates, so the save gives the scope a new epoch first (signed access 3d).
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
    carol.checked_access(team).await.unwrap();

    let organization = admin.organization();
    for (round, named) in [None, Some([9; 32]), Some(before_revocation)]
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
        carol.refresh_keys(None).await.unwrap();
        assert!(
            carol.keys().trusted(made_up),
            "the made-up key did not open, so the test shows nothing"
        );

        let epochs = carol.keys().keys_of(team, KeyKind::Scope).len();
        let text_now = format!("after {round}");
        head = carol
            .save(note, &TextBlock::of(&text_now), head)
            .await
            .unwrap()
            .published();
        assert_eq!(
            carol.keys().keys_of(team, KeyKind::Scope).len(),
            epochs + 1,
            "the save wrote under a key opened through an outdated epoch (round {round})"
        );
        let written = carol.keys().keys_of(note, KeyKind::Data).last().unwrap().id;
        assert_ne!(written, made_up);
        bob.refresh_keys(None).await.unwrap();
        assert!(!bob.keys().has(written));
    }

    harness.stop().await;
}
