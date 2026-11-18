use super::*;

/// The server, with whoever holds a contact's old phrase, serves the account's block holding
/// the old record of the contact again and hides the key change; the device keeps the newest key
/// it saw, so it never seals to the old one again (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn a_contacts_key_the_server_rolls_back_stays_the_newest_one_seen() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let stale = guest
        .refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == owner.account())
        .and_then(|contact| contact.record())
        .unwrap();
    let old = owner.keys().identity().public();
    assert_eq!(guest.keys().contact(owner.account()), Some(old));

    // The owner takes a new phrase; the guest follows the change it sealed with the old one.
    let fresh = new_phrase(
        &owner.connection(),
        owner.account(),
        owner.keys().identity(),
    )
    .await;
    guest.refresh_contacts().await.unwrap();
    assert_eq!(guest.keys().contact(owner.account()), Some(fresh.public()));

    // The block is served holding the old record again, and the change is dropped.
    crate::AccountRecords::open(
        &guest.connection(),
        guest.account(),
        guest.keys().identity(),
    )
    .await
    .unwrap()
    .expect("a person's account has its own block")
    .update(|_| {
        vec![be_block::AccountBlock::set_contact(
            owner.account(),
            Some(stale.kept()),
        )]
    })
    .await
    .unwrap();
    harness
        .database()
        .execute(
            "DELETE FROM mailbox WHERE account_id = ?1 AND sender_id = ?2",
            [guest.account().to_string(), owner.account().to_string()],
        )
        .unwrap();
    let listed = guest.refresh_contacts().await.unwrap();
    assert_eq!(
        guest.keys().contact(owner.account()),
        Some(fresh.public()),
        "the device went back to a key the contact had replaced"
    );
    let contact = listed
        .iter()
        .find(|contact| contact.account == owner.account())
        .unwrap();
    assert_eq!(
        contact.mismatch,
        Some(crate::contacts::Mismatch::Older),
        "the list says the block held an older key"
    );

    harness.stop().await;
}
