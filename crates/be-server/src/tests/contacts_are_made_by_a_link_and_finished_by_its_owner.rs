use super::*;
use be_keys::{CardRole, ContactCard, InviteCode, RecoveryPhrase};

#[tokio::test]
async fn contacts_are_made_by_a_link_and_finished_by_its_owner() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    let mut other = harness.client().await;
    let other_id = other.register("other@example.com").await;
    let personal = owner.personal().await;
    let mut owners_device = harness.second_connection(&owner.token, personal).await;

    let (owner_secret, other_secret) = (
        RecoveryPhrase::generate().secret(),
        RecoveryPhrase::generate().secret(),
    );
    let card = |account: Uuid, secret: &be_keys::RecoverySecret, email: &str| ContactCard {
        account: account.into_bytes(),
        public: secret.public(),
        display_name: email.into(),
        email: email.into(),
    };
    let owner_card = card(owner_id, &owner_secret, "owner@example.com");
    let other_card = card(other_id, &other_secret, "other@example.com");
    let code = InviteCode::generate();
    let link = Uuid::from_bytes(code.lookup());
    let response = owner
        .call(|peer| {
            client::contacts::create_contact_link(
                peer,
                link,
                owner_card.seal(&code, CardRole::Owner),
                owner_secret.seal_own(&code.to_bytes()),
                None,
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let own = owner
        .call(|peer| client::contacts::get_contact_link(peer, link))
        .await;
    assert!(
        matches!(own, Err(_)),
        "the owner opened its own link: {own:?}"
    );
    let Ok(found) = other
        .call(|peer| client::contacts::get_contact_link(peer, link))
        .await
    else {
        panic!("the other account could not read the link");
    };
    assert_eq!(found.owner, owner_id);
    let opened = ContactCard::open(&code, CardRole::Owner, &found.card).unwrap();
    assert_eq!(opened, owner_card);

    let response = other
        .call(|peer| {
            client::contacts::accept_contact_link(
                peer,
                link,
                other_card.seal(&code, CardRole::Accepter),
                opened.seal_own(&other_secret),
                Vec::new(),
            )
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(
        matches!(
            owners_device.next_notification().await,
            Notice::ContactsChanged
        ),
        "the owner's other device was not told"
    );
    let again = harness
        .member("third@example.com")
        .await
        .accept(link, &[])
        .await;
    assert!(refused_as(&again, ErrorCode::LinkNotFound), "{again:?}");

    let theirs = other.contacts().await;
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].account, owner_id);
    assert_eq!(
        ContactCard::open_own(&other_secret, &theirs[0].sealed).unwrap(),
        owner_card
    );
    let mine = owners_device.contacts().await;
    assert_eq!(mine.len(), 1);
    assert!(
        mine[0].sealed.is_empty(),
        "the owner's row was written for it"
    );

    let Ok(links) = owners_device
        .call(|peer| client::contacts::list_contact_links(peer))
        .await
    else {
        panic!("listing links failed");
    };
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].accepter, Some(other_id));
    let code = InviteCode::from_bytes(&owner_secret.open_own(&links[0].code).unwrap()).unwrap();
    let accepted = ContactCard::open(&code, CardRole::Accepter, &links[0].card).unwrap();
    assert_eq!(accepted, other_card);
    let response = owners_device
        .call(|peer| {
            client::contacts::put_contact(peer, other_id, accepted.seal_own(&owner_secret))
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let _ = owners_device
        .call(|peer| client::contacts::delete_contact_link(peer, link))
        .await;
    let Ok(links) = owner
        .call(|peer| client::contacts::list_contact_links(peer))
        .await
    else {
        panic!("listing links failed");
    };
    assert!(links.is_empty());
    let mine = owner.contacts().await;
    assert_eq!(
        ContactCard::open_own(&owner_secret, &mine[0].sealed).unwrap(),
        other_card
    );

    harness.stop().await;
}
