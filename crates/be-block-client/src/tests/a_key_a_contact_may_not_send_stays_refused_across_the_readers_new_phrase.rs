use super::*;

/// As `a_key_a_contact_may_not_send_stays_refused_once_sealed_again_after_its_key_change`, a
/// contact who is no administrator seals the reader a data key of a block it never shared with
/// it (so the reader holds it no other way), which the reader refuses; then
/// the reader takes a new phrase, which seals every key sealed to it again
/// (`contacts::new_identity`): the contact's still as the contact's, so a device on the new
/// phrase refuses it too, never taking it as the reader's own.
#[tokio::test]
async fn a_key_a_contact_may_not_send_stays_refused_across_the_readers_new_phrase() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    befriend(&contact, &reader).await;
    let block = note(&contact).await;
    let to = contact.verified_identity(reader.account()).unwrap();
    let data = contact
        .keys()
        .keys_of(block, KeyKind::Data)
        .last()
        .unwrap()
        .id;
    let wrap = contact
        .keys()
        .seal_to(data, reader.account(), &to.public)
        .unwrap();
    plant_wrap(&harness, contact.organization(), &wrap);
    let (_, planted) = wrap_to(&harness, data, reader.account());
    reader
        .keys()
        .absorb(served_wrap(&harness, &contact, data, reader.account()));
    assert!(
        !reader.keys().has(data),
        "a data key from a contact who is no administrator opened"
    );

    let fresh = new_phrase(
        &reader.connection(),
        reader.account(),
        reader.keys().identity(),
    )
    .await;
    let (sender, sealed) = wrap_to(&harness, data, reader.account());
    assert_ne!(
        sealed, planted,
        "the new phrase did not seal the contact's key again"
    );
    assert_eq!(
        sender,
        contact.account(),
        "the new phrase sealed the contact's key again as another's"
    );
    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh,
            Credentials::Token(reader.token().to_owned()),
        )
        .organization(Some(reader.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    renewed.refresh_contacts().await.unwrap();
    renewed
        .keys()
        .absorb(served_wrap(&harness, &contact, data, reader.account()));
    assert!(
        !renewed.keys().has(data),
        "the key the contact may not send opened on the new phrase"
    );

    harness.stop().await;
}
