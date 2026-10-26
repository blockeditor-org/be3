use super::*;

use be_graph::Access;
use be_keys::Party;
use be_protocol::{ClientMessage, ServerMessage};
use text_block_types::{TextBlock, TextContent};

async fn note_in(peer: &Peer<MemoryStore>, parent: Uuid, body: &str) -> Uuid {
    let note = peer
        .create::<TextContent>(BlockParent::Block(parent))
        .await
        .unwrap();
    peer.save(note, &TextBlock::of(body), None).await.unwrap();
    note
}

async fn with_identity(
    harness: &Harness,
    peer: &Peer<MemoryStore>,
    identity: be_keys::RecoverySecret,
) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            &harness.url,
            identity,
            Credentials::Token(peer.token().to_owned()),
        )
        .organization(Some(peer.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn contacts_keep_sharing_after_one_of_them_takes_a_new_recovery_phrase() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let stranger = harness.member(&owner, "stranger@example.com").await;

    let team = workspace(&owner, "Team").await;
    let from_owner = note_in(&owner, team, "from the owner").await;
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(team, guest.account(), Access::Edit, &public)
        .await
        .unwrap();
    let theirs = workspace(&guest, "Guest's").await;
    let from_guest = note_in(&guest, theirs, "from the guest").await;
    let public = guest.keys().contact(owner.account()).unwrap();
    guest
        .share(theirs, owner.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, from_owner).await, "from the owner");
    assert_eq!(text(&owner, from_guest).await, "from the guest");

    let fresh = be_keys::RecoveryPhrase::generate().secret();
    let forged = stranger.keys().identity().seal_key_change(
        owner.account().into_bytes(),
        &Party {
            account: guest.account().into_bytes(),
            public: guest.keys().identity().public(),
        },
        stranger.keys().identity().public(),
    );
    harness
        .database()
        .execute(
            "INSERT INTO key_changes (account_id, contact_id, position, sealed)
             VALUES (?1, ?2, -1, ?3)",
            rusqlite::params![
                owner.account().to_string(),
                guest.account().to_string(),
                forged
            ],
        )
        .unwrap();
    let ServerMessage::RecoveryKey { sealed, own, .. } = owner
        .connection()
        .request(|request| ClientMessage::GetRecoveryKey { request })
        .await
        .unwrap()
    else {
        panic!("the server did not answer with the recovery key");
    };
    let resealed = crate::contacts::new_identity(
        owner.account(),
        owner.keys().identity(),
        &fresh,
        sealed,
        own,
    )
    .expect("the old phrase opens everything sealed to the owner");
    assert_eq!(resealed.changes.len(), 2, "one change per verified contact");
    owner
        .connection()
        .request(|request| ClientMessage::SetRecoveryKey {
            request,
            public: fresh.public().0,
            sealed: resealed.sealed,
            own: resealed.own,
            changes: resealed.changes,
        })
        .await
        .unwrap();

    let owner = with_identity(&harness, &owner, fresh.clone()).await;
    let guest = harness.second(&guest).await;
    assert_eq!(
        guest.keys().contact(owner.account()),
        Some(fresh.public()),
        "the guest took the owner's new key from the change it sealed with its old one"
    );
    assert_eq!(text(&guest, from_owner).await, "from the owner");
    assert_eq!(text(&owner, from_guest).await, "from the guest");
    assert_eq!(
        text(&harness.second(&guest).await, from_owner).await,
        "from the owner",
        "another device of the guest reads what the owner sealed with its old key"
    );

    let later = workspace(&owner, "Later").await;
    let from_new_owner = note_in(&owner, later, "after the new phrase").await;
    let public = owner.keys().contact(guest.account()).unwrap();
    owner
        .share(later, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, from_new_owner).await, "after the new phrase");
    let back = workspace(&guest, "Back").await;
    let from_new_guest = note_in(&guest, back, "to the new key").await;
    let public = guest.keys().contact(owner.account()).unwrap();
    guest
        .share(back, owner.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&owner, from_new_guest).await, "to the new key");

    harness.stop().await;
}
