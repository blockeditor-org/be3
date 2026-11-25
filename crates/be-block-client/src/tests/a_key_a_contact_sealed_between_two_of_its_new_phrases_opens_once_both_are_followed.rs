use super::*;

use be_block_graph::Access;
use be_type_workspace::FolderContent;

/// A contact takes a new phrase, shares a folder from it, and takes another before the reader
/// follows either. The folder's key is sealed from the contact's middle key, which the reader
/// never holds: following both changes at once, it seals again what each key the changes pass
/// through sealed (`contacts::reseal_from`), so the key opens by the contact's newest.
#[tokio::test]
async fn a_key_a_contact_sealed_between_two_of_its_new_phrases_opens_once_both_are_followed() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    befriend(&contact, &reader).await;

    let middle = new_phrase(
        &contact.connection(),
        contact.account(),
        contact.keys().identity(),
    )
    .await;
    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            be_block_keys::RecoverySecret::from_bytes(middle.to_bytes()),
            Credentials::Token(contact.token().to_owned()),
        )
        .organization(Some(contact.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    renewed.refresh_contacts().await.unwrap();
    let folder = renewed
        .create::<FolderContent>(BlockParent::Root)
        .await
        .unwrap();
    let to = renewed.verified_identity(reader.account()).unwrap();
    renewed
        .share(folder, reader.account(), Access::View, &to)
        .await
        .unwrap();
    let key = renewed.keys().scope_key(folder).unwrap();
    new_phrase(&renewed.connection(), contact.account(), &middle).await;

    let contacts = reader.refresh_contacts().await.unwrap();
    assert!(
        contacts
            .iter()
            .any(|listed| listed.account == contact.account() && listed.public.is_some()),
        "the reader did not follow both of the contact's changes: {contacts:?}"
    );
    reader.refresh_keys(None).await.unwrap();
    assert_eq!(
        reader.keys().scope_key(folder),
        Some(key),
        "the key sealed from the contact's middle key did not open"
    );

    harness.stop().await;
}
