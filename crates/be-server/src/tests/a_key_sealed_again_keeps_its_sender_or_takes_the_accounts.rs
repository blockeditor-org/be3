use super::*;
use be_block_protocol::SealedKey;

/// What the server holds of `key` sealed to `account`: its sender and sealed bytes.
fn held(harness: &Harness, key: Uuid, account: Uuid) -> Option<(String, Vec<u8>)> {
    use rusqlite::OptionalExtension;
    metadata_database(&harness.directory)
        .query_row(
            "SELECT sender, sealed FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [key.to_string(), format!("account:{account}")],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .unwrap()
}

/// An account seals again a key sealed to it (`put-keys.reseal`) as from the sender the row
/// names (a contact's key stays the contact's) or as from itself, never as from a third
/// account, and its own row never as a contact's; a row the server no longer holds is skipped,
/// not refused, and nothing is made of it.
#[tokio::test]
async fn a_key_sealed_again_keeps_its_sender_or_takes_the_accounts() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let account = owner.id();
    let organization = owner.organization("notes").await;
    let own = owner.scope_root(account, BlockParent::Root).await;
    let theirs = owner.scope_root(account, BlockParent::Root).await;
    let (contact, third) = (Uuid::new_v4(), Uuid::new_v4());
    // As a contact's seal of it would read.
    metadata_database(&harness.directory)
        .execute(
            "UPDATE wrapped_keys SET sender = ?1 WHERE key_id = ?2 AND wrapped_by = ?3",
            [
                contact.to_string(),
                theirs.scope.to_string(),
                format!("account:{account}"),
            ],
        )
        .unwrap();
    let reseal = |key: Uuid, sender: Uuid, sealed: &[u8]| {
        be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Reseal(vec![SealedKey {
            organization,
            key,
            sender,
            sealed: sealed.to_vec(),
        }]))
    };

    let refused = owner
        .call(|peer| peer.write::<()>(reseal(theirs.scope, third, b"as a third's")))
        .await;
    assert!(refused.is_err(), "sealed again as from a third account");
    let refused = owner
        .call(|peer| peer.write::<()>(reseal(own.scope, contact, b"as the contact's")))
        .await;
    assert!(refused.is_err(), "the account's own key sealed again as a contact's");
    assert_eq!(
        held(&harness, own.scope, account),
        Some((account.to_string(), b"scope sealed to its author".to_vec()))
    );

    let response = owner
        .call(|peer| peer.write::<()>(reseal(theirs.scope, contact, b"still the contact's")))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(
        held(&harness, theirs.scope, account),
        Some((contact.to_string(), b"still the contact's".to_vec()))
    );

    let gone = Uuid::new_v4();
    let response = owner
        .call(|peer| peer.write::<()>(reseal(gone, account, b"of a key gone")))
        .await;
    assert!(matches!(response, Ok(())), "a gone row refused: {response:?}");
    assert_eq!(held(&harness, gone, account), None);

    harness.stop().await;
}
