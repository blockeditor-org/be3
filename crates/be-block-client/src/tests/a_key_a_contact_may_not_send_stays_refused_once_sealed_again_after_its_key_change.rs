use super::*;

/// A contact who is no administrator shares a block with the reader, and seals the block's data
/// key to the reader too, which only an
/// administrator hands on (`KeyBook::sender_may`), and the server stores it, as a lying server
/// with a colluding contact would. The reader refuses it. The contact then takes a new phrase,
/// and the reader, following the change, seals again what the contact's old key sealed
/// (`contacts::reseal_from`): still as the contact's, so it stays refused, never laundered into
/// a key of the reader's own, which its devices trust unchecked.
#[tokio::test]
async fn a_key_a_contact_may_not_send_stays_refused_once_sealed_again_after_its_key_change() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    befriend(&contact, &reader).await;
    let block = note(&contact).await;
    let to = contact.verified_identity(reader.account()).unwrap();
    // Shared, so the reader's device is told of what is sealed to it there; the reader's own
    // device here takes only what the test hands it.
    contact
        .share(block, reader.account(), be_block_graph::Access::View, &to)
        .await
        .unwrap();
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

    new_phrase(
        &contact.connection(),
        contact.account(),
        contact.keys().identity(),
    )
    .await;
    reader.refresh_contacts().await.unwrap();
    let (sender, sealed) = wrap_to(&harness, data, reader.account());
    assert_ne!(
        sealed, planted,
        "what the contact's old key sealed was not sealed again"
    );
    assert_eq!(
        sender,
        contact.account(),
        "the contact's key was sealed again as another's"
    );
    reader
        .keys()
        .absorb(served_wrap(&harness, &contact, data, reader.account()));
    assert!(
        !reader.keys().has(data),
        "the key the contact may not send opened once sealed again"
    );

    harness.stop().await;
}
