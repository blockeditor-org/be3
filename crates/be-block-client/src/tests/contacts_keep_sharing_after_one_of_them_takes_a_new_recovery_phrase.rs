use super::*;

use be_block_graph::Access;
use be_block_keys::Party;
use be_block_protocol::RecoveryKey;
use be_type_text::{TextBlock, TextContent};

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
    identity: be_block_keys::RecoverySecret,
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
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(team, guest.account(), Access::Edit, &public)
        .await
        .unwrap();
    let theirs = workspace(&guest, "Guest's").await;
    let from_guest = note_in(&guest, theirs, "from the guest").await;
    let public = guest.verified_identity(owner.account()).unwrap();
    guest
        .share(theirs, owner.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, from_owner).await, "from the owner");
    assert_eq!(text(&owner, from_guest).await, "from the guest");

    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let forged = stranger.keys().identity().seal_key_change(
        owner.account().into_bytes(),
        &Party {
            account: guest.account().into_bytes(),
            public: guest.keys().identity().public(),
        },
        stranger.keys().identity().identity(),
    );
    // A forged change ahead of the real one in the guest's mailbox, as the owner's.
    harness
        .database()
        .execute(
            "INSERT INTO mailbox (account_id, sender_id, kind, letter, created)
             VALUES (?1, ?2, 'key-change', ?3, unixepoch())",
            rusqlite::params![
                guest.account().to_string(),
                owner.account().to_string(),
                be_block_protocol::encode(&be_block_protocol::Letter::KeyChange(forged)).unwrap()
            ],
        )
        .unwrap();
    let RecoveryKey {
        sealed, own, logs, ..
    } = crate::ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            &owner.connection(),
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await
    .expect("the server answers with the recovery key");
    let block = Uuid::new_v4();
    let before = owner.sign_authored(block, b"before");
    let old = owner.keys().identity().clone();
    let resealed = crate::contacts::new_identity(
        owner.account(),
        owner.keys().identity(),
        &fresh,
        sealed,
        own,
        logs,
    )
    .expect("the old phrase opens everything sealed to the owner");
    assert_eq!(resealed.changes.len(), 2, "one change per verified contact");
    crate::ask(be_block_protocol::write::<(), _>(
        &owner.connection(),
        None,
        be_block_protocol::Op::SetRecoveryKey {
            public: fresh.public().0,
            signing: fresh.signing().0,
            sealed: resealed.sealed,
            own: resealed.own,
            changes: resealed.changes,
            retirements: resealed.retirements,
        },
    ))
    .await
    .unwrap();

    let owner = with_identity(&harness, &owner, fresh.clone()).await;
    let watcher = guest;
    let guest = harness.second(&watcher).await;
    assert_eq!(
        guest.verified_identity(owner.account()),
        Some(fresh.identity()),
        "the guest took the owner's new key from the change it sealed with its old one"
    );
    assert_eq!(
        guest
            .verify_authored(
                owner.account(),
                block,
                b"after",
                &owner.sign_authored(block, b"after"),
                None,
            )
            .await
            .unwrap(),
        Signer::Verified,
        "the change carried the owner's new signing key too"
    );
    watcher.refresh_contacts().await.unwrap();
    assert_eq!(
        watcher
            .verify_authored(owner.account(), block, b"before", &before, None)
            .await
            .unwrap(),
        Signer::Changed,
        "what the old key signed still verifies, but the old key is no longer the owner's"
    );
    let now = watcher
        .server_now()
        .expect("the watcher measured the server's clock");
    assert_eq!(
        watcher
            .verify_authored(owner.account(), block, b"before", &before, Some(0))
            .await
            .unwrap(),
        Signer::Changed,
        "a record the server took before the watcher learned of the new key is the old key's"
    );
    assert_eq!(
        watcher
            .verify_authored(
                owner.account(),
                block,
                b"before",
                &before,
                Some(now + 2 * 60 * 1000),
            )
            .await
            .unwrap(),
        Signer::Replaced,
        "one the server took after is louder than a changed key, but not called forged"
    );
    // The owner's phone, not yet told of the new phrase, still signs with the old key.
    let from_phone = old.sign_authored(block.into_bytes(), b"from the phone");
    assert_eq!(
        watcher
            .verify_authored(
                owner.account(),
                block,
                b"from the phone",
                &from_phone,
                Some(now + 2 * 60 * 1000),
            )
            .await
            .unwrap(),
        Signer::Replaced,
        "an honest device behind on the new phrase is not called forged"
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
    let public = owner.verified_identity(guest.account()).unwrap();
    owner
        .share(later, guest.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&guest, from_new_owner).await, "after the new phrase");
    let back = workspace(&guest, "Back").await;
    let from_new_guest = note_in(&guest, back, "to the new key").await;
    let public = guest.verified_identity(owner.account()).unwrap();
    guest
        .share(back, owner.account(), Access::View, &public)
        .await
        .unwrap();
    assert_eq!(text(&owner, from_new_guest).await, "to the new key");

    harness.stop().await;
}
