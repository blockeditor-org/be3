use super::*;

/// The guest accepts Rosa's link and Rosa removes the guest before any device of the guest's
/// took the record: one pass takes both the record and the removal, and keeps the removal with
/// that record's `verified`. The server then sends that record again, and an older one: neither
/// brings Rosa back, on that device or another, until the two connect again by a link.
#[tokio::test]
async fn a_contact_taken_and_removed_in_one_pass_stays_removed_whatever_is_replayed() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.owner("guest@example.com").await;
    let rosa_card = crate::contacts::card(
        rosa.account(),
        rosa.keys().identity(),
        "Rosa",
        "rosa@example.com",
    );
    let code =
        crate::contacts::make_link(&rosa.connection(), rosa.keys().identity(), &rosa_card, None)
            .await
            .unwrap();
    crate::contacts::accept_link(
        &guest.connection(),
        guest.keys().identity(),
        &crate::contacts::card(
            guest.account(),
            guest.keys().identity(),
            "Guest",
            "guest@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
        &Default::default(),
    )
    .await
    .unwrap();
    rosa.finish_links().await.unwrap();
    // What the server kept of the record the accepting write left the guest.
    let letter = crate::contacts::mailbox(&guest.connection())
        .await
        .unwrap()
        .into_iter()
        .find_map(|message| match message.letter {
            be_block_protocol::Letter::Contact(sealed) => Some(sealed),
            _ => None,
        })
        .expect("the guest's record of Rosa waits in its mailbox");
    rosa.remove_contact(guest.account()).await.unwrap();

    let listed = guest.refresh_contacts().await.unwrap();
    assert!(listed.is_empty(), "the removal was not taken: {listed:?}");

    let older = crate::contacts::OwnContact::new(rosa_card, 0);
    let older = crate::contacts::OwnContact {
        verified: 1,
        ..older
    };
    for replayed in [letter, older.seal(guest.keys().identity())] {
        harness
            .database()
            .execute(
                "INSERT INTO mailbox (account_id, sender_id, kind, letter, created)
                 VALUES (?1, ?2, 'contact', ?3, unixepoch())",
                rusqlite::params![
                    guest.account().to_string(),
                    rosa.account().to_string(),
                    be_block_protocol::encode(&be_block_protocol::Letter::Contact(replayed))
                        .unwrap()
                ],
            )
            .unwrap();
        let listed = guest.refresh_contacts().await.unwrap();
        assert!(
            listed.is_empty(),
            "a replayed record brought a removed contact back: {listed:?}"
        );
    }
    let laptop = harness.second(&guest).await;
    assert!(
        laptop.refresh_contacts().await.unwrap().is_empty(),
        "another device took a replayed record"
    );

    befriend(&rosa, &guest).await;
    assert!(
        guest
            .refresh_contacts()
            .await
            .unwrap()
            .iter()
            .any(|contact| contact.account == rosa.account() && contact.public.is_some()),
        "a new link after the removal was not taken"
    );

    harness.stop().await;
}
