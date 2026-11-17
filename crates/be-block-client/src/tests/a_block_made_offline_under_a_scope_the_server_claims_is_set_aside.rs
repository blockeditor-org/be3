use super::*;

use be_block_graph::Access;

/// A block made where the signed parent records cannot be read (offline, or a read that does
/// not answer) takes the scope its parent's kept summary names, unchecked, and goes up only
/// through the outbox, which checks it first (signed access 4): one made under a folder the
/// server claimed lies in a contact's workspace, where the owner holds the contact's scope
/// key, is set aside as misplaced, its keys wrapped there never sent.
#[tokio::test]
async fn a_block_made_offline_under_a_scope_the_server_claims_is_set_aside() {
    let mut harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let contact = harness.member(&owner, "contact@example.com").await;
    let theirs = workspace(&contact, "Theirs").await;
    let identity = contact.verified_identity(owner.account()).unwrap();
    contact
        .share(theirs, owner.account(), Access::Edit, &identity)
        .await
        .unwrap();
    let private = workspace(&owner, "Private").await;
    let folder = owner
        .create::<TextContent>(BlockParent::Block(private))
        .await
        .unwrap();

    // The server claims the folder moved into the contact's workspace.
    harness
        .database()
        .execute(
            "UPDATE blocks SET parent_kind = 2, parent_id = ?1
             WHERE organization_id = ?2 AND id = ?3",
            [
                theirs.to_string(),
                owner.organization().to_string(),
                folder.to_string(),
            ],
        )
        .unwrap();
    harness.server.restart().await;
    harness.url = format!("ws://{}", harness.server.address);
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let laptop = harness.device(&owner, &refs, &store).await;
    assert_eq!(
        laptop.summary(folder).await.unwrap().scope,
        Some(theirs),
        "the laptop keeps the scope the server claims"
    );
    laptop.refresh_keys(None).await.unwrap();
    drop(laptop);

    let offline = harness.offline(&owner, &refs, &store);
    let note = offline
        .create::<TextContent>(BlockParent::Block(folder))
        .await
        .expect("a block is made offline");
    drop(offline);

    let online = harness.device(&owner, &refs, &store).await;
    let _ = online.drain(|_| false).await;
    let unsent = online.unsent();
    assert!(
        unsent
            .iter()
            .any(|unsent| unsent.block == note && unsent.refused == Some(Aside::Misplaced)),
        "the block made under the claimed scope was not set aside as misplaced: {unsent:?}"
    );
    let wrapped: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM keys WHERE block_id = ?1",
            [note.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(wrapped, 0, "the misplaced block's keys went up");

    harness.stop().await;
}
