use super::*;

use be_block_graph::Access;

/// Puts a scope key of `block` with id `key` in the server's database, sealed to `to` by
/// `sender` as `sealed`.
fn plant(harness: &Harness, organization: Uuid, key: Uuid, block: Uuid, to: Uuid, sender: Uuid, sealed: Vec<u8>) {
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', 90, 0)",
            rusqlite::params![organization.to_string(), key.to_string(), block.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                format!("account:{to}"),
                sender.to_string(),
                sealed
            ],
        )
        .unwrap();
}

/// A key a contact seals, of a block whose log never names them, is taken only when they hold
/// Edit or above on a block the signed parent records place it under (signed access 4): with
/// the server, a contact with no right anywhere above hands over no made-up scope key; an
/// editor of the workspace above does.
#[tokio::test]
async fn a_contact_the_log_never_names_hands_over_a_key_only_with_edit_from_above() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let editor = harness.member(&owner, "editor@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;
    let team = workspace(&owner, "Team").await;
    owner
        .grant(team, editor.account(), Access::Edit)
        .await
        .unwrap();
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    // A scope of its own, whose log names the owner alone.
    owner.offer(note, Access::View).await.unwrap();
    let (_, checked) = owner.checked_access(note).await.unwrap();
    assert!(!checked.levels.contains_key(&editor.account().into_bytes()));

    let organization = owner.organization();
    let mut planted = Vec::new();
    for sender in [&editor, &stranger] {
        let key = Uuid::new_v4();
        let carried = CarriedKey {
            key: [7; 32],
            block: note.into_bytes(),
            root: None,
            heads: Heads {
                block: Some(checked.head),
                organization: None,
            },
        };
        let sealed = sender.keys().identity().seal_for(
            sender.account().into_bytes(),
            &Party {
                account: owner.account().into_bytes(),
                public: owner.keys().identity().public(),
            },
            key.as_bytes(),
            &carried.encode(),
        );
        plant(
            &harness,
            organization,
            key,
            note,
            owner.account(),
            sender.account(),
            sealed,
        );
        planted.push(key);
    }

    // A device of the owner new to the note, which pins its root from the owner's own key.
    let device = another(&harness, &owner).await;
    device.refresh_keys(None).await.unwrap();
    assert!(
        device.keys().has(planted[0]),
        "an editor of the workspace above handed over no key of the note"
    );
    assert!(
        !device.keys().has(planted[1]),
        "a contact with no right above handed over a key of the note"
    );

    harness.stop().await;
}
