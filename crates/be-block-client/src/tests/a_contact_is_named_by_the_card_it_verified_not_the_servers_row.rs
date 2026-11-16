use super::*;

/// The server labels one verified contact with another's name; the device names each by the
/// card it verified, so sharing with "Bob" can't seal to Mallory (plans/crypto_audit.md, 5).
#[tokio::test]
async fn a_contact_is_named_by_the_card_it_verified_not_the_servers_row() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let bob = harness.member(&owner, "bob@example.com").await;
    let mallory = harness.member(&owner, "mallory@example.com").await;
    harness
        .database()
        .execute(
            "UPDATE accounts SET display_name = 'bob@example.com', email = NULL WHERE id = ?1",
            [mallory.account().to_string()],
        )
        .unwrap();
    let listed = owner.refresh_contacts().await.unwrap();
    let named = |account: Uuid| {
        let contact = listed
            .iter()
            .find(|contact| contact.account == account)
            .unwrap();
        (contact.display_name.clone(), contact.email.clone())
    };
    assert_eq!(
        named(mallory.account()),
        (
            "mallory@example.com".to_owned(),
            "mallory@example.com".to_owned()
        ),
        "the server's row named Mallory as Bob"
    );
    assert_eq!(
        named(bob.account()),
        ("bob@example.com".to_owned(), "bob@example.com".to_owned())
    );
    assert_eq!(
        owner.contact_name(mallory.account()).as_deref(),
        Some("mallory@example.com"),
        "the name kept for offline is the card's too"
    );

    harness.stop().await;
}
