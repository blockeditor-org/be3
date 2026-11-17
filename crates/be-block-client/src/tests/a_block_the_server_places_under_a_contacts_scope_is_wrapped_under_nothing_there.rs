use super::*;

use be_block_graph::Access;
use be_block_protocol::KeyWork;
use be_type_text::TextContent;

/// How many of `block`'s keys the server holds wrapped under a key of `scope`.
fn wrapped_under(database: &rusqlite::Connection, block: Uuid, scope: Uuid) -> i64 {
    database
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys
             JOIN keys ON keys.organization_id = wrapped_keys.organization_id
                AND keys.id = wrapped_keys.key_id
             WHERE keys.block_id = ?1
                AND wrapped_keys.wrapped_by IN (SELECT id FROM keys WHERE block_id = ?2)",
            [block.to_string(), scope.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

/// Which block is under which is the signed parent records' word, not the server's (signed
/// access 4): a server that claims an owner's note moved into a workspace a contact manages,
/// where the owner holds the contact's scope key, gets none of the note's keys wrapped under
/// it, by the next rotation or by a block made under the note.
#[tokio::test]
async fn a_block_the_server_places_under_a_contacts_scope_is_wrapped_under_nothing_there() {
    let mut harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let contact = harness.member(&owner, "contact@example.com").await;

    // The contact's workspace, which the owner may edit, so holds its scope key.
    let theirs = workspace(&contact, "Theirs").await;
    let identity = contact.verified_identity(owner.account()).unwrap();
    contact
        .share(theirs, owner.account(), Access::Edit, &identity)
        .await
        .unwrap();
    owner.refresh_keys(None).await.unwrap();
    assert!(
        owner.keys().scope_key(theirs).is_some(),
        "the owner holds the contact's scope key"
    );

    // The owner's own note, in a workspace no one else reads.
    let private = workspace(&owner, "Private").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(private))
        .await
        .unwrap();
    let database = harness.database();
    assert_eq!(wrapped_under(&database, note, theirs), 0);

    // The server claims the note moved into the contact's workspace.
    database
        .execute(
            "UPDATE blocks SET parent_kind = 2, parent_id = ?1
             WHERE organization_id = ?2 AND id = ?3",
            [
                theirs.to_string(),
                owner.organization().to_string(),
                note.to_string(),
            ],
        )
        .unwrap();
    harness.server.restart().await;
    harness.url = format!("ws://{}", harness.server.address);
    harness.reconnect(&owner, owner.token()).await;
    let claimed = owner.summary(note).await.unwrap();
    assert_eq!(claimed.parent, Some(BlockParent::Block(theirs)));
    assert_eq!(
        claimed.scope,
        Some(theirs),
        "the server says the note lies in the contact's scope"
    );

    // The next rotation of the note wraps nothing under the contact's keys.
    let work = KeyWork {
        rotations: vec![note],
        ..owner.key_work().await.unwrap()
    };
    let rotated = owner.tend_with(work).await;
    assert!(
        matches!(rotated, Err(ClientError::Forged(_))),
        "a rotation under a scope the note was never placed in went ahead: {rotated:?}"
    );
    assert_eq!(
        wrapped_under(&database, note, theirs),
        0,
        "a rotation wrapped the note's keys under the contact's scope"
    );

    // Nor does a block made under the note go up wrapped there.
    let made = owner.create::<TextContent>(BlockParent::Block(note)).await;
    assert!(
        matches!(made, Err(ClientError::Forged(_))),
        "a block was made under the scope the server claims: {made:?}"
    );

    harness.stop().await;
}
