use super::*;



fn waiting(harness: &Harness, to: Uuid, from: Uuid) -> i64 {
    harness
        .database()
        .query_row(
            "SELECT count(*) FROM mailbox
             WHERE account_id = ?1 AND sender_id = ?2 AND kind = 'key-change'",
            [to.to_string(), from.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

/// A contact takes a new phrase while the guest has no device connected: the change waits in
/// the guest's mailbox, and the guest's next device follows it from there and deletes it.
#[tokio::test]
async fn a_key_change_reaches_a_contact_offline_and_is_read_from_its_mailbox() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    guest.refresh_contacts().await.unwrap();
    assert_eq!(
        guest.keys().contact(owner.account()),
        Some(owner.keys().identity().public())
    );
    let (account, identity, token, organization) = (
        guest.account(),
        guest.keys().identity().clone(),
        guest.token().to_owned(),
        guest.organization(),
    );
    guest.detach();
    drop(guest);

    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    take_phrase(&owner.connection(), owner.account(), owner.keys().identity(), &fresh).await;
    assert_eq!(
        waiting(&harness, account, owner.account()),
        1,
        "the change waits in the guest's mailbox, from the owner"
    );

    let guest = Peer::connect(
        PeerConfig::new(&harness.url, identity, Credentials::Token(token))
            .organization(Some(organization))
            .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    guest.refresh_contacts().await.unwrap();
    assert_eq!(
        guest.verified_identity(owner.account()),
        Some(fresh.identity()),
        "the guest's device took the owner's new key from its mailbox"
    );
    assert_eq!(
        waiting(&harness, account, owner.account()),
        0,
        "the change it followed left the mailbox"
    );

    harness.stop().await;
}
