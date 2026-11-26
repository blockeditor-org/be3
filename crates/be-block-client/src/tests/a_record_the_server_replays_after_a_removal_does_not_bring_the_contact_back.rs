use super::*;

/// The server keeps every record a device ever sent itself in a letter, and can say a contact was
/// removed. After a removal it replays the old record: the account's block keeps the removal
/// with the newest record it had reached, so the old card is not taken, and the contact stays
/// gone until the two connect again by a link, whose record is newer.
#[tokio::test]
async fn a_record_the_server_replays_after_a_removal_does_not_bring_the_contact_back() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.owner("guest@example.com").await;
    befriend(&rosa, &guest).await;
    let old = guest
        .refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == rosa.account())
        .and_then(|contact| contact.record())
        .expect("the guest holds a record of Rosa");
    // What the server kept of the letter that brought it.
    let replayed = old.seal(guest.keys().identity());

    rosa.remove_contact(guest.account()).await.unwrap();
    let listed = guest.refresh_contacts().await.unwrap();
    assert!(listed.is_empty(), "the removal was not taken: {listed:?}");

    harness
        .database()
        .execute(
            "INSERT INTO mailbox (account_id, sender_id, kind, letter, created)
             VALUES (?1, ?2, 'contact', ?3, unixepoch())",
            rusqlite::params![
                guest.account().to_string(),
                rosa.account().to_string(),
                be_block_protocol::encode(&be_block_protocol::Letter::Contact(replayed)).unwrap()
            ],
        )
        .unwrap();
    let listed = guest.refresh_contacts().await.unwrap();
    assert!(
        listed.is_empty(),
        "a replayed record brought a removed contact back: {listed:?}"
    );
    let laptop = harness.second(&guest).await;
    assert!(
        laptop.refresh_contacts().await.unwrap().is_empty(),
        "another device took the replayed record"
    );

    befriend(&rosa, &guest).await;
    let listed = guest.refresh_contacts().await.unwrap();
    assert!(
        listed
            .iter()
            .any(|contact| contact.account == rosa.account() && contact.public.is_some()),
        "a new link after the removal was not taken: {listed:?}"
    );

    harness.stop().await;
}
