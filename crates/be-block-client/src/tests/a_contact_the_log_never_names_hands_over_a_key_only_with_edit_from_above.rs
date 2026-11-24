use super::*;

use be_block_graph::Access;

/// Puts a scope key of `block` with id `key` in the server's database, sealed to `to` by
/// `sender` as `sealed`.
fn plant(
    harness: &Harness,
    organization: Uuid,
    key: Uuid,
    block: Uuid,
    to: Uuid,
    sender: Uuid,
    sealed: Vec<u8>,
) {
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
/// Edit or above on a block the signed parent records place it under: with
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
    owner.offer_scope(note, Access::View).await.unwrap();
    let (_, checked) = owner.checked_access(note).await.unwrap();
    assert!(!checked.levels.contains_key(&editor.account().into_bytes()));

    let organization = owner.organization();
    let seal = |sender: &Peer<MemoryStore>, key: Uuid, head: EntryHash| {
        let carried = CarriedKey {
            key: [7; 32],
            label: crate::tests::planted(key, "scope", 90),
            block: note.into_bytes(),
            root: None,
            heads: Heads {
                block: Some(head),
                organization: None,
            },
        };
        sender
            .keys()
            .identity()
            .seal_for(
                sender.account().into_bytes(),
                &Party {
                    account: owner.account().into_bytes(),
                    public: owner.keys().identity().public(),
                },
                key.as_bytes(),
                &carried.encode(),
            )
            .unwrap()
    };
    let mut planted = Vec::new();
    let mut seals = Vec::new();
    for sender in [&editor, &stranger] {
        let key = Uuid::new_v4();
        let carried = CarriedKey {
            key: [7; 32],
            label: crate::tests::planted(key, "scope", 90),
            block: note.into_bytes(),
            root: None,
            heads: Heads {
                block: Some(checked.head),
                organization: None,
            },
        };
        let sealed = sender
            .keys()
            .identity()
            .seal_for(
                sender.account().into_bytes(),
                &Party {
                    account: owner.account().into_bytes(),
                    public: owner.keys().identity().public(),
                },
                key.as_bytes(),
                &carried.encode(),
            )
            .unwrap();
        plant(
            &harness,
            organization,
            key,
            note,
            owner.account(),
            sender.account(),
            sealed.clone(),
        );
        planted.push(key);
        seals.push(sealed);
    }

    // A device of the owner new to the note, which pins its root from the owner's own key.
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let device = harness.device(&owner, &refs, &store).await;
    device.refresh_keys(None).await.unwrap();
    assert!(
        device.keys().has(planted[0]),
        "an editor of the workspace above handed over no key of the note"
    );
    assert!(
        !device.keys().has(planted[1]),
        "a contact with no right above handed over a key of the note"
    );

    // Revoked above, the editor hands over nothing new, naming the newest head or not.
    owner
        .grant(team, editor.account(), Access::None)
        .await
        .unwrap();
    let (_, now) = owner.checked_access(note).await.unwrap();
    let made_up = Uuid::new_v4();
    plant(
        &harness,
        organization,
        made_up,
        note,
        owner.account(),
        editor.account(),
        seal(&editor, made_up, now.head),
    );
    device.refresh_keys(None).await.unwrap();
    assert!(
        !device.keys().has(made_up),
        "an editor revoked above handed over a key made up since"
    );
    assert!(
        device.keys().has(planted[0]),
        "the key vouched for as it arrived is kept"
    );
    drop(device);

    // A device that vouched for the editor's wrap of a key, and is then served another unnamed
    // contact's wrap of the same key, made up: the vouch names the wrap it was found for, so
    // this one is judged afresh, and refused.
    let fresh = harness
        .device(&owner, &refs::RefsDb::memory(), &MemoryStore::new())
        .await;
    fresh.checked_access(note).await.unwrap();
    assert!(
        fresh
            .keys()
            .vouch(note, planted[0], editor.account(), &seals[0])
            .is_some()
    );
    let swapped = {
        let carried = CarriedKey {
            key: [9; 32],
            label: crate::tests::planted(planted[0], "scope", 90),
            block: note.into_bytes(),
            root: None,
            heads: Heads {
                block: Some(checked.head),
                organization: None,
            },
        };
        stranger
            .keys()
            .identity()
            .seal_for(
                stranger.account().into_bytes(),
                &Party {
                    account: owner.account().into_bytes(),
                    public: owner.keys().identity().public(),
                },
                planted[0].as_bytes(),
                &carried.encode(),
            )
            .unwrap()
    };
    harness
        .database()
        .execute(
            "UPDATE wrapped_keys SET sender = ?1, sealed = ?2 WHERE key_id = ?3",
            rusqlite::params![
                stranger.account().to_string(),
                swapped,
                planted[0].to_string()
            ],
        )
        .unwrap();
    fresh.refresh_keys(None).await.unwrap();
    assert!(
        !fresh.keys().has(planted[0]),
        "another contact's wrap of a vouched key was taken"
    );

    harness.stop().await;
}
