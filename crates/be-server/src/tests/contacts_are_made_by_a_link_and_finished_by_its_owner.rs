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
        .send(|request| ClientMessage::CreateContactLink {
            request,
            link,
            card: owner_card.seal(&code, CardRole::Owner),
            code: owner_secret.seal_own(&code.to_bytes()),
            grant: None,
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");

    let own = owner
        .send(|request| ClientMessage::GetContactLink { request, link })
        .await;
    assert!(
        matches!(own, ServerMessage::Failed { .. }),
        "the owner opened its own link: {own:?}"
    );
    let ServerMessage::ContactLink { link: found, .. } = other
        .send(|request| ClientMessage::GetContactLink { request, link })
        .await
    else {
        panic!("the other account could not read the link");
    };
    assert_eq!(found.owner, owner_id);
    let opened = ContactCard::open(&code, CardRole::Owner, &found.card).unwrap();
    assert_eq!(opened, owner_card);

    let response = other
        .send(|request| ClientMessage::AcceptContactLink {
            request,
            link,
            card: other_card.seal(&code, CardRole::Accepter),
            contact: opened.seal_own(&other_secret),
            key: Vec::new(),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    assert!(
        matches!(
            owners_device.next_notification().await,
            ServerMessage::ContactsChanged
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

    let ServerMessage::ContactLinks { links, .. } = owners_device
        .send(|request| ClientMessage::ListContactLinks { request })
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
        .send(|request| ClientMessage::PutContact {
            request,
            contact: other_id,
            sealed: accepted.seal_own(&owner_secret),
        })
        .await;
    assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
    owners_device
        .send(|request| ClientMessage::DeleteContactLink { request, link })
        .await;
    let ServerMessage::ContactLinks { links, .. } = owner
        .send(|request| ClientMessage::ListContactLinks { request })
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
