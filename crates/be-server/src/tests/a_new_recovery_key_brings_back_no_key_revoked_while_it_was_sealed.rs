use super::*;
use be_block_protocol::SealedKey;

/// A device reads every key sealed to its account, seals them again to a new phrase and sends
/// them; meanwhile a contact's share to the account is revoked. The new phrase updates the rows
/// still held in place and brings back none revoked since the device read them, and keeps a
/// contact's row it did not seal again as it was.
#[tokio::test]
async fn a_new_recovery_key_brings_back_no_key_revoked_while_it_was_sealed() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let contact = harness.member("contact@example.com").await.id();
    let account = owner.id();
    let organization = owner.organization("notes").await;
    let own = owner.scope_root(account, BlockParent::Root).await;
    let revoked = owner.scope_root(account, BlockParent::Root).await;
    let kept = owner.scope_root(account, BlockParent::Root).await;
    let by = format!("account:{account}");
    let database = metadata_database(&harness.directory);
    // Two of the rows are taken as a contact's, sealed to the account.
    for key in [revoked.scope, kept.scope] {
        database
            .execute(
                "UPDATE wrapped_keys SET sender = ?1 WHERE key_id = ?2 AND wrapped_by = ?3",
                rusqlite::params![contact.to_string(), key.to_string(), by],
            )
            .unwrap();
    }

    let old = test_secret(account);
    let new = be_block_keys::RecoveryPhrase::generate().secret();
    let found = owner
        .call(|peer| {
            peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey)
        })
        .await
        .unwrap();
    assert_eq!(found.sealed.len(), 3);
    // Revoked after the device read it.
    database
        .execute(
            "DELETE FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            rusqlite::params![revoked.scope.to_string(), by],
        )
        .unwrap();
    let retirements = found
        .logs
        .into_iter()
        .map(|log| be_block_protocol::Retirement {
            organization: log.organization,
            kind: log.kind,
            log: log.log,
            entry: be_block_keys::Entry::retirement(
                &old,
                &new,
                account.into_bytes(),
                log.log.into_bytes(),
                log.head.try_into().unwrap(),
            )
            .encode(),
        })
        .collect();
    let resealed = |key: Uuid, sender: Uuid| SealedKey {
        organization,
        key,
        sender,
        sealed: format!("{key} resealed").into_bytes(),
    };
    let identity = new.identity();
    let response = owner
        .call(|peer| {
            peer.write::<()>(be_block_protocol::Op::SetRecoveryKey {
                public: identity.public.0,
                signing: identity.signing.0,
                // The contact's kept row is not sealed again: the device could not follow them.
                sealed: vec![
                    resealed(own.scope, account),
                    resealed(revoked.scope, contact),
                ],
                changes: Vec::new(),
                retirements,
            })
        })
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    remember_secret(account, new);

    let rows: Vec<(String, String, Vec<u8>)> = database
        .prepare("SELECT key_id, sender, sealed FROM wrapped_keys WHERE wrapped_by = ?1 ORDER BY key_id")
        .unwrap()
        .query_map([&by], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let held = |key: Uuid| rows.iter().find(|row| row.0 == key.to_string());
    assert_eq!(
        held(revoked.scope),
        None,
        "the new phrase brought back a key revoked while it was sealed"
    );
    assert_eq!(
        held(own.scope).map(|row| (row.1.clone(), row.2.clone())),
        Some((account.to_string(), format!("{} resealed", own.scope).into_bytes())),
        "the account's own row was not updated in place"
    );
    assert_eq!(
        held(kept.scope).map(|row| row.1.clone()),
        Some(contact.to_string()),
        "the contact's row not sealed again was dropped"
    );
    assert_ne!(
        held(kept.scope).map(|row| row.2.clone()),
        Some(format!("{} resealed", kept.scope).into_bytes())
    );

    harness.stop().await;
}
