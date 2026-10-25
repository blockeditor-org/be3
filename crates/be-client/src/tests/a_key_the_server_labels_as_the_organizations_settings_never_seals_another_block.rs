use super::*;

use be_keys::{Party, RecoverySecret};
use be_store::ContentKey;
use text_block_types::{TextBlock, TextContent};

fn insert_key(
    database: &rusqlite::Connection,
    organization: Uuid,
    block: Uuid,
    kind: &str,
) -> Uuid {
    let key = Uuid::new_v4();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, ?4, 99, 0)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                block.to_string(),
                kind
            ],
        )
        .unwrap();
    key
}

fn insert_wrap(
    database: &rusqlite::Connection,
    organization: Uuid,
    key: Uuid,
    wrapped_by: String,
    sender: Uuid,
    sealed: Vec<u8>,
) {
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                wrapped_by,
                sender.to_string(),
                sealed
            ],
        )
        .unwrap();
}

#[tokio::test]
async fn a_key_the_server_labels_as_the_organizations_settings_never_seals_another_block() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let victim = harness.member(&admin, "victim@example.com").await;
    let organization = admin.organization();
    let private = workspace(&victim, "Private").await;
    let note = victim
        .create::<TextContent>(BlockParent::Block(private))
        .await
        .unwrap();
    let head = victim
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published();
    let honest = victim.current_key(note).unwrap();

    let database = harness.database();
    let (server, secret): (String, Vec<u8>) = database
        .query_row(
            "SELECT account_id, secret FROM server_account WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let server: Uuid = server.parse().unwrap();
    let secret = RecoverySecret::from_bytes(secret.try_into().unwrap());
    let scope_plain = [7u8; 32];
    let data_plain = [8u8; 32];
    let scope = insert_key(
        &database,
        organization,
        admin.organization_settings(),
        "scope",
    );
    insert_wrap(
        &database,
        organization,
        scope,
        format!("account:{}", victim.account()),
        server,
        secret.seal_for(
            server.into_bytes(),
            &Party {
                account: victim.account().into_bytes(),
                public: victim.keys().identity().public(),
            },
            scope.as_bytes(),
            &scope_plain,
        ),
    );
    let data = insert_key(&database, organization, note, "data");
    insert_wrap(
        &database,
        organization,
        data,
        scope.to_string(),
        server,
        ContentKey::from_bytes(scope_plain).seal(data, &data_plain),
    );

    let reopened = harness.second(&victim).await;
    reopened.refresh_keys(None).await.unwrap();
    assert!(
        !reopened.keys().trusted(scope) && !reopened.keys().trusted(data),
        "a key only the server vouched for was trusted"
    );
    reopened
        .save(note, &TextBlock::of("second"), head)
        .await
        .unwrap();
    let sealed = reopened.current_key(note).unwrap();
    assert_ne!(
        sealed, data,
        "the note was sealed with a key the server knows"
    );
    assert!(sealed == honest || reopened.keys().trusted(sealed));
    assert_eq!(text(&victim, note).await, "second");

    harness.stop().await;
}
