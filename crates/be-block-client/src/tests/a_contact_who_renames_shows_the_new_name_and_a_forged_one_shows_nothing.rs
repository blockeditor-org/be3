use super::*;

/// A contact renamed in the server's portal: its next connection is greeted with the new name,
/// and publishes it as a name card its signing key signs, so the account's contacts show the
/// new name without a new link. A card the server makes up shows nothing, and the server takes
/// none that the account's signing key did not sign (guides/blocks/keys.md, "Contacts").
#[tokio::test]
async fn a_contact_who_renames_shows_the_new_name_and_a_forged_one_shows_nothing() {
    let harness = Harness::start().await;
    let rosa = harness.owner("rosa@example.com").await;
    let guest = harness.member(&rosa, "guest@example.com").await;
    rosa.publish_name().await.unwrap();
    let named = |contacts: Vec<crate::Contact>| {
        contacts
            .into_iter()
            .find(|contact| contact.account == rosa.account())
            .map(|contact| contact.display_name)
            .unwrap()
    };
    assert!(
        guest
            .refresh_contacts()
            .await
            .unwrap()
            .iter()
            .any(|contact| contact.account == rosa.account() && contact.signing.is_some()),
        "the guest verified rosa"
    );

    harness
        .database()
        .execute(
            "UPDATE accounts SET display_name = 'Rosa Renamed' WHERE id = ?1",
            [rosa.account().to_string()],
        )
        .unwrap();
    let renamed = harness.second(&rosa).await;
    renamed.publish_name().await.unwrap();
    assert_eq!(
        named(guest.refresh_contacts().await.unwrap()),
        "Rosa Renamed",
        "the contact's new name reached the guest"
    );

    // A card for rosa signed by a key not hers: the server refuses to keep it, and a server
    // that serves it anyway names her as it was.
    let forged = be_block_keys::NameCard {
        account: rosa.account().into_bytes(),
        display_name: "Forged".into(),
        email: "forged@example.com".into(),
        dated: u64::MAX / 2,
    }
    .sign(&identity("mallory@example.com"));
    assert!(
        be_block_protocol::write::<(), _>(
            &renamed.connection(),
            Some(renamed.organization()),
            be_block_protocol::Op::PublishName(forged.clone()),
        )
        .await
        .unwrap()
        .is_err(),
        "the server kept a card her key did not sign"
    );
    let rosa_account = rosa.account();
    harness.server.lie(guest.account(), move |page| {
        if let be_block_protocol::Page::SigningKeys(keys) = page {
            for key in keys.iter_mut().filter(|key| key.account == rosa_account) {
                key.name = forged.clone();
            }
        }
    });
    assert_ne!(
        named(guest.refresh_contacts().await.unwrap()),
        "Forged",
        "a name card the contact's key did not sign named them"
    );

    harness.stop().await;
}
