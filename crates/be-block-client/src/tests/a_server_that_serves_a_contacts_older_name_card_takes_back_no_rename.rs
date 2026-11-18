use super::*;

/// A contact renames, and this device shows the name card their key signed. A server that
/// later serves an older card they signed, or none, takes the rename back on no device that
/// showed it: each keeps the newest card it showed (`Refs::name_card`).
#[tokio::test]
async fn a_server_that_serves_a_contacts_older_name_card_takes_back_no_rename() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    harness
        .database()
        .execute(
            "UPDATE accounts SET display_name = 'Rosa Renamed' WHERE id = ?1",
            [rosa.account().to_string()],
        )
        .unwrap();
    harness.second(&rosa).await.publish_name().await.unwrap();
    let named = |contacts: Vec<crate::Contact>| {
        contacts
            .into_iter()
            .find(|contact| contact.account == rosa.account())
            .map(|contact| contact.display_name)
            .unwrap()
    };
    assert_eq!(named(guest.refresh_contacts().await.unwrap()), "Rosa Renamed");

    // A card rosa did sign, dated before the rename, then none at all.
    let older = be_block_keys::NameCard {
        account: rosa.account().into_bytes(),
        display_name: "Rosa Before".into(),
        email: "rosa@example.com".into(),
        dated: 1,
    }
    .sign(rosa.keys().identity());
    let serving = std::sync::Arc::new(std::sync::Mutex::new(older));
    let served = std::sync::Arc::clone(&serving);
    let rosa_account = rosa.account();
    harness.server.lie(guest.account(), move |page| {
        if let be_block_protocol::Page::SigningKeys(keys) = page {
            for key in keys.iter_mut().filter(|key| key.account == rosa_account) {
                key.name = served.lock().unwrap().clone();
            }
        }
    });
    assert_eq!(
        named(guest.refresh_contacts().await.unwrap()),
        "Rosa Renamed",
        "an older card rosa signed took the rename back"
    );
    *serving.lock().unwrap() = Vec::new();
    assert_eq!(
        named(guest.refresh_contacts().await.unwrap()),
        "Rosa Renamed",
        "a server serving no card took the rename back"
    );

    harness.stop().await;
}
