use super::*;
use be_block_keys::{CardRole, ContactCard, InviteCode, RecoveryPhrase};

#[tokio::test]
async fn contacts_are_made_by_a_link_and_finished_by_its_owner() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    let mut other = harness.member("other@example.com").await;
    let other_id = other.id();
    let personal = owner.personal().await;
    let mut owners_device = harness.second_connection(&owner.token, personal).await;

    let (owner_secret, other_secret) = (
        RecoveryPhrase::generate().secret(),
        RecoveryPhrase::generate().secret(),
    );
    let card = |account: Uuid, secret: &be_block_keys::RecoverySecret, email: &str| ContactCard {
        account: account.into_bytes(),
        public: secret.public(),
        signing: secret.signing(),
        display_name: email.into(),
        email: email.into(),
    };
    let owner_card = card(owner_id, &owner_secret, "owner@example.com");
    let other_card = card(other_id, &other_secret, "other@example.com");
    let code = InviteCode::generate();
    let link = Uuid::from_bytes(code.lookup());
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink {
                link: link,
                card: owner_card.seal(&code, CardRole::Owner),
                kind: be_block_protocol::LinkKind::Contact(None),
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");

    let own = owner
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await;
    assert!(
        matches!(own, Err(_)),
        "the owner opened its own link: {own:?}"
    );
    let Ok(found) = other
        .call(|peer| peer.read::<be_block_protocol::Link>(be_block_protocol::Query::Link(link)))
        .await
    else {
        panic!("the other account could not read the link");
    };
    assert_eq!(found.owner, owner_id);
    let opened = ContactCard::open(&code, CardRole::Owner, &found.card).unwrap();
    assert_eq!(opened, owner_card);

    let response = other
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink {
                link: link,
                card: other_card.seal(&code, CardRole::Accepter),
                contact: other_secret.seal_own(&opened.encode()),
                key: Vec::new(),
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert!(
        matches!(
            owners_device.next_notification().await,
            Notice::MailboxChanged
        ),
        "the owner's other device was not told"
    );
    let again = harness
        .member("third@example.com")
        .await
        .accept(link, &[])
        .await;
    assert!(refused_as(&again, ErrorCode::LinkNotFound), "{again:?}");

    // The server keeps only that the two are contacts; the accepter's record of the owner waits
    // in its own mailbox, for a device to keep in its block.
    assert_eq!(other.contacts().await, vec![owner_id]);
    assert_eq!(owners_device.contacts().await, vec![other_id]);
    let Ok(theirs) = other
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::Message>>(be_block_protocol::Query::Mailbox)
        })
        .await
    else {
        panic!("reading the mailbox failed");
    };
    let [
        be_block_protocol::Message {
            from: Some(from),
            letter: be_block_protocol::Letter::Contact(record),
            ..
        },
    ] = &theirs[..]
    else {
        panic!("the accepter's mailbox holds no record alone: {theirs:?}");
    };
    assert_eq!(*from, owner_id);
    assert_eq!(
        ContactCard::decode(&other_secret.open_own(record).unwrap()).unwrap(),
        owner_card
    );

    let links = owners_device.links().await;
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].accepter, Some(other_id));
    // The accepter's card waits in the owner's mailbox, from the accepter.
    let Ok(mailbox) = owners_device
        .call(|peer| {
            peer.read::<Vec<be_block_protocol::Message>>(be_block_protocol::Query::Mailbox)
        })
        .await
    else {
        panic!("reading the mailbox failed");
    };
    let [
        be_block_protocol::Message {
            id: letter,
            from: Some(from),
            letter: be_block_protocol::Letter::Accepted { link: of, card },
            ..
        },
    ] = &mailbox[..]
    else {
        panic!("the mailbox holds no accepted card alone: {mailbox:?}");
    };
    assert_eq!((*from, *of), (other_id, link));
    let accepted = ContactCard::open(&code, CardRole::Accepter, card).unwrap();
    let letter = *letter;
    assert_eq!(accepted, other_card);
    let _ = owners_device
        .call(|peer| {
            peer.write_all::<()>(vec![
                be_block_protocol::Op::DeleteLink(link),
                be_block_protocol::Op::DeleteMessage(letter),
            ])
        })
        .await;
    assert!(owner.links().await.is_empty());
    assert_eq!(owner.contacts().await, vec![other_id]);

    harness.stop().await;
}
