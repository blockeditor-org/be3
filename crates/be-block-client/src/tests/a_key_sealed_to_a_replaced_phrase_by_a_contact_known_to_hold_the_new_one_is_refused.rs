use super::*;

use be_type_workspace::FolderContent;

/// The reader takes a new phrase, and its contact follows it and then takes a new phrase of its
/// own, sealing its change to the reader's new key: the reader's record of the contact holds
/// the current key (`OwnContact::holds`). A key then sealed "from" the contact to the reader's
/// old key, as whoever holds the old secret could make one, is refused: a retired secret opens
/// only what a contact that may still hold the old key seals.
#[tokio::test]
async fn a_key_sealed_to_a_replaced_phrase_by_a_contact_known_to_hold_the_new_one_is_refused() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    befriend(&contact, &reader).await;
    let old = reader.keys().identity().public();

    let fresh = new_phrase(
        &reader.connection(),
        reader.account(),
        reader.keys().identity(),
    )
    .await;
    contact.refresh_contacts().await.unwrap();
    assert_eq!(
        contact
            .verified_identity(reader.account())
            .map(|identity| identity.public),
        Some(fresh.public()),
        "the contact did not follow the reader's change"
    );
    let theirs = new_phrase(
        &contact.connection(),
        contact.account(),
        contact.keys().identity(),
    )
    .await;
    let connect = |secret: be_block_keys::RecoverySecret, peer: &Peer<MemoryStore>| {
        Peer::connect(
            PeerConfig::new(
                &harness.url,
                secret,
                Credentials::Token(peer.token().to_owned()),
            )
            .organization(Some(peer.organization())),
            MemoryStore::new(),
        )
    };
    let reader = connect(fresh, &reader).await.unwrap();
    let contact = connect(theirs, &contact).await.unwrap();
    reader.refresh_contacts().await.unwrap();
    contact.refresh_contacts().await.unwrap();
    assert_eq!(
        reader
            .verified_identity(contact.account())
            .map(|identity| identity.public),
        Some(contact.keys().identity().public()),
        "the reader did not follow the contact's change"
    );

    let folder = contact
        .create::<FolderContent>(BlockParent::Root)
        .await
        .unwrap();
    let key = contact.keys().scope_key(folder).unwrap();
    let wrap = contact.keys().seal_to(key, reader.account(), &old).unwrap();
    plant_wrap(&harness, contact.organization(), &wrap);
    reader
        .keys()
        .absorb(served_wrap(&harness, &contact, key, reader.account()));
    assert_eq!(
        reader.keys().scope_key(folder),
        None,
        "a retired secret opened a key from a contact known to hold the current one"
    );

    harness.stop().await;
}
