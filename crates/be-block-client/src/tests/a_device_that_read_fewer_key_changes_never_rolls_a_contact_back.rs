use super::*;

use be_block_keys::RecoverySecret;
use be_block_protocol::RecoveryKey;

/// Takes a new phrase for `peer`'s account, leaving a key change in each contact's mailbox.
async fn new_phrase(peer: &Peer<MemoryStore>) -> RecoverySecret {
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let RecoveryKey {
        sealed, own, logs, ..
    } = crate::ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            &peer.connection(),
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await
    .unwrap();
    let resealed = crate::contacts::new_identity(
        peer.account(),
        peer.keys().identity(),
        &fresh,
        sealed,
        own,
        logs,
    )
    .unwrap();
    crate::ask(be_block_protocol::write::<(), _>(
        &peer.connection(),
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
    fresh
}

/// The owner takes two new phrases. One device of the guest read only the first change and is
/// slow to write what follows it; the other read both, follows them and deletes them first. The
/// slow write lands last and is refused, since the record it replaces is no longer the one it
/// read, so the guest never goes back to the owner's middle key, which nothing leads on from.
#[tokio::test]
async fn a_device_that_read_fewer_key_changes_never_rolls_a_contact_back() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let fast = harness.member(&owner, "guest@example.com").await;
    let slow = harness.second(&fast).await;
    let pinned = slow
        .refresh_contacts()
        .await
        .unwrap()
        .into_iter()
        .find(|contact| contact.account == owner.account())
        .and_then(|contact| contact.record())
        .unwrap();
    let row = |harness: &Harness| -> Vec<u8> {
        harness
            .database()
            .query_row(
                "SELECT sealed FROM contacts WHERE account_id = ?1 AND contact_id = ?2",
                [fast.account().to_string(), owner.account().to_string()],
                |row| row.get(0),
            )
            .unwrap()
    };
    let read = row(&harness);

    let first = new_phrase(&owner).await;
    let letter: i64 = harness
        .database()
        .query_row(
            "SELECT id FROM mailbox WHERE account_id = ?1 AND sender_id = ?2",
            [fast.account().to_string(), owner.account().to_string()],
            |row| row.get(0),
        )
        .unwrap();
    // What the slow device would write having followed the first change alone.
    let mut card = pinned.card.clone();
    card.public = first.public();
    card.signing = first.signing();
    let stale = crate::contacts::OwnContact::new(card, pinned.verified);

    let owner = Peer::connect(
        PeerConfig::new(
            &harness.url,
            first.clone(),
            Credentials::Token(owner.token().to_owned()),
        )
        .organization(Some(owner.organization()))
        .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let second = new_phrase(&owner).await;
    fast.refresh_contacts().await.unwrap();
    assert_eq!(
        fast.verified_identity(owner.account()),
        Some(second.identity())
    );

    let landed = crate::ask(be_block_protocol::write_all::<(), _>(
        &slow.connection(),
        None,
        vec![
            be_block_protocol::Op::PutContact {
                contact: owner.account(),
                sealed: stale.seal(slow.keys().identity()),
                replaces: read,
            },
            be_block_protocol::Op::DeleteMessage(letter as u64),
        ],
    ))
    .await;
    assert!(
        matches!(
            landed,
            Err(ClientError::Refused(ErrorCode::RecordChanged, _))
        ),
        "the slow write went over the newer record: {landed:?}"
    );
    let held = crate::contacts::OwnContact::open(
        fast.keys().identity(),
        &row(&harness),
        owner.account(),
    )
    .unwrap();
    assert_eq!(held.card.public, second.public(), "the record rolled back");
    slow.refresh_contacts().await.unwrap();
    assert_eq!(
        slow.verified_identity(owner.account()),
        Some(second.identity()),
        "the slow device, reading again, follows to the newest key"
    );

    harness.stop().await;
}
