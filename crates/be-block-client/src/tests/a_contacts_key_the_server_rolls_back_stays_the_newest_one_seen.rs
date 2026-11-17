use super::*;

use be_block_protocol::RecoveryKey;

/// The server, with whoever holds a contact's old phrase, serves the old own-sealed card for
/// the contact again and hides the key change; the device keeps the newest key it saw, so it
/// never seals to the old one again (plans/crypto_audit.md, 4).
#[tokio::test]
async fn a_contacts_key_the_server_rolls_back_stays_the_newest_one_seen() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    guest.refresh_contacts().await.unwrap();
    let old = owner.keys().identity().public();
    assert_eq!(guest.keys().contact(owner.account()), Some(old));
    let stale: Vec<u8> = harness
        .database()
        .query_row(
            "SELECT sealed FROM contacts WHERE account_id = ?1 AND contact_id = ?2",
            [guest.account().to_string(), owner.account().to_string()],
            |row| row.get(0),
        )
        .unwrap();

    // The owner takes a new phrase; the guest follows the change it sealed with the old one.
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
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
    .unwrap();
    let resealed = crate::contacts::new_identity(
        owner.account(),
        owner.keys().identity(),
        &fresh,
        sealed,
        own,
        logs,
    )
    .unwrap();
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
    guest.refresh_contacts().await.unwrap();
    assert_eq!(guest.keys().contact(owner.account()), Some(fresh.public()));

    // The server puts the old card back and drops the change.
    let database = harness.database();
    database
        .execute(
            "UPDATE contacts SET sealed = ?3 WHERE account_id = ?1 AND contact_id = ?2",
            rusqlite::params![
                guest.account().to_string(),
                owner.account().to_string(),
                stale
            ],
        )
        .unwrap();
    database
        .execute(
            "DELETE FROM mailbox WHERE account_id = ?1 AND sender_id = ?2",
            [guest.account().to_string(), owner.account().to_string()],
        )
        .unwrap();
    let listed = guest.refresh_contacts().await.unwrap();
    assert_eq!(
        guest.keys().contact(owner.account()),
        Some(fresh.public()),
        "the device went back to a key the contact had replaced"
    );
    let contact = listed
        .iter()
        .find(|contact| contact.account == owner.account())
        .unwrap();
    assert_eq!(
        contact.mismatch,
        Some(crate::contacts::Mismatch::Older),
        "the list says the server served an older key"
    );

    harness.stop().await;
}
