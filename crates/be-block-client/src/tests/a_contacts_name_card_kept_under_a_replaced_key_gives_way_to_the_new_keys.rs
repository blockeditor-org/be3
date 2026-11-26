use super::*;

/// A card rosa's old key signed, dated far ahead (a stolen key, or a clock running fast), is
/// what the guest shows and keeps. Rosa takes a new phrase and publishes a card dated now: the
/// guest shows that one, as the card kept was opened with a key rosa no longer holds.
#[tokio::test]
async fn a_contacts_name_card_kept_under_a_replaced_key_gives_way_to_the_new_keys() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    let named = |contacts: Vec<crate::Contact>| {
        contacts
            .into_iter()
            .find(|contact| contact.account == rosa.account())
            .map(|contact| contact.display_name)
            .unwrap()
    };

    let ahead = be_block_keys::NameCard {
        account: rosa.account().into_bytes(),
        display_name: "Rosa Far Ahead".into(),
        email: "rosa@example.com".into(),
        dated: u64::MAX / 2,
    }
    .sign(rosa.keys().identity());
    let serving = std::sync::Arc::new(std::sync::Mutex::new(Some(ahead)));
    let served = std::sync::Arc::clone(&serving);
    let rosa_account = rosa.account();
    harness.server.lie(guest.account(), move |page| {
        if let be_block_protocol::Page::SigningKeys(keys) = page
            && let Some(card) = served.lock().unwrap().clone()
        {
            for key in keys.iter_mut().filter(|key| key.account == rosa_account) {
                key.name = card.clone();
            }
        }
    });
    assert_eq!(
        named(guest.refresh_contacts().await.unwrap()),
        "Rosa Far Ahead"
    );
    *serving.lock().unwrap() = None;

    let fresh = new_phrase(&rosa.connection(), rosa.account(), rosa.keys().identity()).await;
    harness
        .database()
        .execute(
            "UPDATE accounts SET display_name = 'Rosa New' WHERE id = ?1",
            [rosa.account().to_string()],
        )
        .unwrap();
    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh.clone(),
            Credentials::Token(rosa.token().to_owned()),
        )
        .organization(Some(rosa.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    renewed.publish_name().await.unwrap();

    let listed = guest.refresh_contacts().await.unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|contact| contact.account == rosa.account())
            .and_then(|contact| contact.signing),
        Some(fresh.signing()),
        "the guest followed rosa's new phrase"
    );
    assert_eq!(
        named(listed),
        "Rosa New",
        "a card kept under rosa's old key outranked what her new key signs"
    );

    harness.stop().await;
}
