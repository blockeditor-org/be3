use super::*;

use be_block_keys::CarriedKey;
use be_type_text::{TextBlock, TextContent};

/// The server holds the settings key, so it can wrap a key it made under it, labelled as anyone's
/// block, name that key the block's current one, and say the block lives under the settings.
/// On a member's device and an
/// administrator's alike, the block is never sealed under it: a key the server holds seals only
/// the settings block itself, whatever the server says about where a block lives.
#[tokio::test]
async fn a_key_the_server_wraps_under_the_settings_key_and_names_current_seals_nothing() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let organization = admin.organization();
    let settings = admin.organization_settings();
    let scope = admin
        .keys()
        .scope_key(settings)
        .expect("the owner holds the settings key");
    let scope_plain = admin.keys().ring().get(scope).unwrap();
    let server: Uuid = harness
        .database()
        .query_row(
            "SELECT account_id FROM server_account WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .parse()
        .unwrap();

    for (who, victim) in [("a member", &member), ("an administrator", &admin)] {
        let note = victim
            .create::<TextContent>(BlockParent::Block(workspace(victim, "Private").await))
            .await
            .unwrap();
        let head = victim
            .save(note, &TextBlock::of("first"), None)
            .await
            .unwrap()
            .published();

        // A key the server made, wrapped under the settings key as the note's, named current.
        let planted = Uuid::new_v4();
        let sealed = {
            let ring = be_block_store::Keyring::new();
            ring.insert(scope, scope_plain, settings);
            let carried = CarriedKey {
                key: [8; 32],
                label: crate::tests::planted(planted, "data", 99),
                block: note.into_bytes(),
                root: None,
                heads: be_block_keys::Heads::default(),
            };
            ring.seal(scope, &carried.encode()).unwrap()
        };
        let database = harness.database();
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
        database
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
        drop(database);

        // It names the key current, and says the note lives under the settings, to every device
        // of the victim's.
        harness.server.lie(victim.account(), move |summary| {
            if summary.id == note {
                summary.key = Some(planted);
                summary.parent = Some(BlockParent::Block(settings));
            }
        });
        let device = harness.second(victim).await;
        device.refresh_keys(None).await.unwrap();
        let summary = device.summary(note).await.unwrap();
        assert_eq!(summary.key, Some(planted), "the server lied");
        device.note(&summary);
        assert!(
            !matches!(device.sealing_key(note).await, Ok(key) if key == planted),
            "{who} would seal the note under the server's key"
        );
        assert!(
            !device.keys().trusted(planted),
            "{who} trusted the server's key"
        );
        let saved = device.save(note, &TextBlock::of("second"), head).await;
        assert!(
            saved.is_err() || device.current_key(note) != Some(planted),
            "{who} sealed the note under the server's key"
        );
    }
}
