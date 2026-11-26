use super::*;

use be_block_graph::Access;
use be_type_workspace::FolderContent;

fn sealed_from(harness: &Harness, to: Uuid, from: Uuid) -> Vec<Vec<u8>> {
    harness
        .database()
        .prepare("SELECT sealed FROM wrapped_keys WHERE wrapped_by = ?1 AND sender = ?2 ORDER BY key_id")
        .unwrap()
        .query_map([format!("account:{to}"), from.to_string()], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn rows_from(harness: &Harness, to: Uuid, from: Uuid) -> i64 {
    harness
        .database()
        .query_row(
            "SELECT count(*) FROM wrapped_keys WHERE wrapped_by = ?1 AND sender = ?2",
            [format!("account:{to}"), from.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

/// A contact takes a new phrase and shares a folder from it, and the reader takes a new phrase
/// of its own before it follows the contact's change (here its letter is held back), so it
/// cannot open the folder's key to seal it again. The server keeps that row rather than drop it,
/// and once the reader follows the contact, the key opens with the phrase it retired.
#[tokio::test]
async fn a_contacts_row_a_new_phrase_could_not_seal_again_outlasts_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let reader = harness.member(&admin, "reader@example.com").await;
    let contact = harness.member(&admin, "contact@example.com").await;
    befriend(&contact, &reader).await;

    let middle = new_phrase(
        &contact.connection(),
        contact.account(),
        contact.keys().identity(),
    )
    .await;
    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            be_block_keys::RecoverySecret::from_bytes(middle.to_bytes()),
            Credentials::Token(contact.token().to_owned()),
        )
        .organization(Some(contact.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    renewed.refresh_contacts().await.unwrap();
    let folder = renewed
        .create::<FolderContent>(BlockParent::Root)
        .await
        .unwrap();
    let to = renewed.verified_identity(reader.account()).unwrap();
    renewed
        .share(folder, reader.account(), Access::View, &to)
        .await
        .unwrap();
    let key = renewed.keys().scope_key(folder).unwrap();
    let sealed = rows_from(&harness, reader.account(), contact.account());
    assert!(sealed > 0, "the share sealed nothing to the reader");

    // The contact's key change, held back from the reader's mailbox.
    let database = harness.database();
    database
        .execute_batch(&format!(
            "CREATE TABLE held AS SELECT * FROM mailbox
             WHERE account_id = '{reader}' AND sender_id = '{contact}' AND kind = 'key-change';
             DELETE FROM mailbox
             WHERE account_id = '{reader}' AND sender_id = '{contact}' AND kind = 'key-change';",
            reader = reader.account(),
            contact = contact.account(),
        ))
        .unwrap();
    let before = sealed_from(&harness, reader.account(), contact.account());
    let fresh = new_phrase(
        &reader.connection(),
        reader.account(),
        reader.keys().identity(),
    )
    .await;
    assert_eq!(
        rows_from(&harness, reader.account(), contact.account()),
        sealed,
        "the new phrase dropped the contact's row it could not seal again"
    );

    database
        .execute_batch("INSERT INTO mailbox SELECT * FROM held; DROP TABLE held;")
        .unwrap();
    let after = Peer::connect(
        PeerConfig::new(
            &harness.url,
            be_block_keys::RecoverySecret::from_bytes(fresh.to_bytes()),
            Credentials::Token(reader.token().to_owned()),
        )
        .organization(Some(reader.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    let contacts = after.refresh_contacts().await.unwrap();
    assert!(
        contacts
            .iter()
            .any(|listed| listed.account == contact.account() && listed.public.is_some()),
        "the reader did not follow the contact's change: {contacts:?}"
    );
    after.refresh_keys(None).await.unwrap();
    assert_eq!(
        after.keys().scope_key(folder),
        Some(key),
        "the key the new phrase could not seal again did not open with the retired one"
    );
    let resealed = sealed_from(&harness, reader.account(), contact.account());
    assert_eq!(resealed.len(), before.len());
    assert!(
        resealed.iter().all(|row| !before.contains(row)),
        "once opened with the retired secret, the row was not sealed again to the new phrase"
    );

    // Sealed to the new phrase now, it is sealed again by the next one like any other row.
    let newest = new_phrase(&after.connection(), reader.account(), &fresh).await;
    let last = Peer::connect(
        PeerConfig::new(
            &harness.url,
            newest,
            Credentials::Token(reader.token().to_owned()),
        )
        .organization(Some(reader.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    last.refresh_contacts().await.unwrap();
    last.refresh_keys(None).await.unwrap();
    assert_eq!(
        last.keys().scope_key(folder),
        Some(key),
        "the key did not open after a second new phrase"
    );

    harness.stop().await;
}
