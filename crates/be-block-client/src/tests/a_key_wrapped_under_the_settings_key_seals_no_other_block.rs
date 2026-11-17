use super::*;

use be_block_keys::CarriedKey;
use be_type_text::{TextBlock, TextContent};

/// The server holds the settings block's scope key, so a key wrapped under it vouches for
/// nothing outside the settings: one the server wraps there, labelled as another block's newest
/// data key, opens untrusted, and the block is still written under a key of its own, even on
/// the device that minted the settings key and trusts it.
#[tokio::test]
async fn a_key_wrapped_under_the_settings_key_seals_no_other_block() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let organization = admin.organization();
    let settings = admin.organization_settings();
    let note = admin
        .create::<TextContent>(BlockParent::Block(workspace(&admin, "Mine").await))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("first"), None)
        .await
        .unwrap()
        .published();
    let scope = admin
        .keys()
        .scope_key(settings)
        .expect("the owner holds the settings key");
    assert!(
        admin.keys().trusted(scope),
        "the owner minted the settings key"
    );

    // The server, which holds the settings key, wraps a key it chose under it as the note's.
    let planted = Uuid::new_v4();
    let carried = CarriedKey {
        key: [8; 32],
        block: note.into_bytes(),
        root: None,
    };
    let sealed = {
        let ring = be_block_store::Keyring::new();
        ring.insert(scope, admin.keys().ring().get(scope).unwrap(), settings);
        ring.seal(scope, &carried.encode()).unwrap()
    };
    let server: Uuid = {
        let database = harness.database();
        let server: String = database
            .query_row(
                "SELECT account_id FROM server_account WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        database
            .execute(
                "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
                 VALUES (?1, ?2, ?3, 'data', 99, 0)",
                rusqlite::params![
                    organization.to_string(),
                    planted.to_string(),
                    note.to_string()
                ],
            )
            .unwrap();
        server.parse().unwrap()
    };
    harness
        .database()
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                planted.to_string(),
                scope.to_string(),
                server.to_string(),
                sealed
            ],
        )
        .unwrap();

    admin.refresh_keys(None).await.unwrap();
    assert!(
        !admin.keys().trusted(planted),
        "a key the server wrapped under the settings key was trusted"
    );
    admin
        .save(note, &TextBlock::of("second"), head)
        .await
        .unwrap();
    assert_ne!(
        admin.current_key(note),
        Some(planted),
        "the note was sealed under a key the server chose"
    );
    assert_eq!(text(&admin, note).await, "second");
}
