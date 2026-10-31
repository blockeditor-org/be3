use super::*;
use be_keys::{CardRole, ContactCard, InviteCode, RecoveryPhrase};

#[tokio::test]
async fn a_contact_card_the_server_swaps_does_not_open() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let mut other = harness.member("other@example.com").await;

    let honest = ContactCard {
        account: owner_id.into_bytes(),
        public: RecoveryPhrase::generate().secret().public(),
        display_name: "Owner".into(),
        email: "owner@example.com".into(),
    };
    let code = InviteCode::generate();
    let link = Uuid::from_bytes(code.lookup());
    let response = owner
        .call(|peer| {
            client::contacts::create_contact_link(
                peer,
                link,
                honest.seal(&code, CardRole::Owner),
                Vec::new(),
                None,
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let forged = ContactCard {
        public: RecoveryPhrase::generate().secret().public(),
        ..honest.clone()
    };
    let database = rusqlite::Connection::open(harness.directory.join("metadata.sqlite")).unwrap();
    database
        .execute(
            "UPDATE contact_links SET card = ?1 WHERE id = ?2",
            rusqlite::params![
                forged.seal(&InviteCode::generate(), CardRole::Owner),
                link.to_string()
            ],
        )
        .unwrap();

    let Ok(found) = other
        .call(|peer| client::contacts::get_contact_link(peer, link))
        .await
    else {
        panic!("the link could not be read");
    };
    assert!(
        ContactCard::open(&code, CardRole::Owner, &found.card).is_err(),
        "a card the server put in place opened with the code"
    );

    harness.stop().await;
}
