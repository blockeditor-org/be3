use super::*;

use be_block_graph::Access;
use be_block_keys::RecoverySecret;
use be_block_protocol::{RecoveryKey, Unsealed};

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
/// lists the owner as not confirmed and seals nothing to them, rather than stay on the old key,
/// and takes what the old key signs as a replaced key's. A new link confirms them again and
/// clears what led nowhere.
#[tokio::test]
async fn a_contact_whose_key_changes_outran_the_mailbox_is_not_confirmed_until_linked_again() {
    let harness = Harness::start().await;
    // The guest administers the organization; the owner, a member, takes the new phrases.
    let guest = harness.owner("guest@example.com").await;
    let owner = harness.member(&guest, "owner@example.com").await;
    guest.refresh_contacts().await.unwrap();
    let account = guest.account();
    let block = Uuid::new_v4();
    let signed = owner.sign_authored(block, b"by the old key");
    let verify = || guest.verify_authored(owner.account(), block, b"by the old key", &signed, None);
    assert_eq!(verify().await.unwrap(), Signer::Verified);

    let mut current = owner.keys().identity().clone();
    let bound = be_server::store::MAX_LETTERS_FROM_ONE;
    for _ in 0..=bound {
        current = new_phrase(&owner.connection(), owner.account(), &current).await;
    }
    assert_eq!(waiting(&harness, account, owner.account()), bound);

    // The guest reads its mailbox only now.
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
    assert_eq!(
        shown.display_name, "owner@example.com",
        "named as it was verified"
    );
    assert_eq!(guest.verified_identity(owner.account()), None);
    assert_eq!(
        waiting(&harness, account, owner.account()),
        bound,
        "changes that lead nowhere are not taken as followed"
    );
    assert_eq!(
        verify().await.unwrap(),
        Signer::Changed,
        "the key the owner replaced still answers as theirs"
    );
    // A share to them, even with the server listing the key as due them, seals nothing.
    let theirs = workspace(&guest, "Guest's").await;
    let scope = guest.keys().scope_key(theirs).unwrap();
    let _ = guest.grant(theirs, owner.account(), Access::View).await;
    let mut work = guest.key_work().await.unwrap();
    work.unsealed.push(Unsealed {
        key: scope,
        account: owner.account(),
    });
    guest.tend_with(work).await.unwrap();
    let wraps: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [scope.to_string(), format!("account:{}", owner.account())],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(wraps, 0, "a wrap names the contact whose chain broke");

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
    assert_eq!(
        guest
            .verify_authored(
                owner.account(),
                block,
                b"by the new key",
                &current.sign_authored(block.into_bytes(), b"by the new key"),
                None,
            )
            .await
            .unwrap(),
        Signer::Verified
    );
    assert_eq!(waiting(&harness, account, owner.account()), 0);

    harness.stop().await;
}
