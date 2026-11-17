use super::*;

use be_block_keys::RecoverySecret;
use be_block_protocol::RecoveryKey;

/// Moves `account` from `old` to a new phrase, leaving a key change in each contact's mailbox.
async fn new_phrase(
    connection: &std::sync::Arc<crate::Connection>,
    account: Uuid,
    old: &RecoverySecret,
) -> RecoverySecret {
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let RecoveryKey {
        sealed, own, logs, ..
    } = crate::ask(
        be_block_protocol::read::<be_block_protocol::RecoveryKey, _>(
            connection,
            None,
            be_block_protocol::Query::RecoveryKey,
        ),
    )
    .await
    .unwrap();
    let resealed = crate::contacts::new_identity(account, old, &fresh, sealed, own, logs).unwrap();
    crate::ask(be_block_protocol::write::<(), _>(
        connection,
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
    .expect("a new phrase never waits on a contact's mailbox");
    fresh
}

fn waiting(harness: &Harness, to: Uuid, from: Uuid) -> i64 {
    harness
        .database()
        .query_row(
            "SELECT count(*) FROM mailbox
             WHERE account_id = ?1 AND sender_id = ?2 AND kind = 'key-change'",
            [to.to_string(), from.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

/// The owner takes more new phrases than the server keeps changes for while the guest is away:
/// every one is taken, the oldest change goes, and the guest's device, finding the chain broken,
/// lists the owner as not confirmed and seals nothing to them, rather than stay on the old key.
/// A new link confirms them again and clears what led nowhere.
#[tokio::test]
async fn a_contact_whose_key_changes_outran_the_mailbox_is_not_confirmed_until_linked_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    guest.refresh_contacts().await.unwrap();
    let (account, identity, token, organization) = (
        guest.account(),
        guest.keys().identity().clone(),
        guest.token().to_owned(),
        guest.organization(),
    );
    guest.detach();
    drop(guest);

    let mut current = owner.keys().identity().clone();
    let bound = be_server::store::MAX_LETTERS_FROM_ONE;
    for _ in 0..=bound {
        current = new_phrase(&owner.connection(), owner.account(), &current).await;
    }
    assert_eq!(waiting(&harness, account, owner.account()), bound);

    let guest = Peer::connect(
        PeerConfig::new(&harness.url, identity, Credentials::Token(token))
            .organization(Some(organization))
            .chunker(ChunkerConfig::SMALL),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let listed = guest.refresh_contacts().await.unwrap();
    let shown = listed
        .iter()
        .find(|contact| contact.account == owner.account())
        .unwrap();
    assert_eq!(
        (shown.public, shown.verified),
        (None, 0),
        "the owner is listed as not confirmed, not on the old key"
    );
    assert_eq!(shown.display_name, "Admin", "named as it was verified");
    assert_eq!(guest.verified_identity(owner.account()), None);
    assert_eq!(
        waiting(&harness, account, owner.account()),
        bound,
        "changes that lead nowhere are not taken as followed"
    );

    // The owner connects again by a link, as the list asks.
    let code = crate::contacts::make_link(
        &owner.connection(),
        &current,
        &crate::contacts::card(owner.account(), &current, "Owner", "owner@example.com"),
        None,
    )
    .await
    .unwrap();
    crate::contacts::accept_link(
        &guest.connection(),
        guest.keys().identity(),
        &crate::contacts::card(
            account,
            guest.keys().identity(),
            "guest@example.com",
            "guest@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
    )
    .await
    .unwrap();
    guest.refresh_contacts().await.unwrap();
    assert_eq!(
        guest.verified_identity(owner.account()),
        Some(current.identity())
    );
    assert_eq!(waiting(&harness, account, owner.account()), 0);

    harness.stop().await;
}
