use be_block_graph::SETTINGS_TYPE;
use be_block_protocol::{OrganizationRole, Unsealed};

use super::*;

/// An owner-only block is its author's alone and never escrowed, so its keys are sealed to
/// nobody else, even another administrator the server lists them as owed to.
#[tokio::test]
async fn an_owner_only_blocks_keys_are_sealed_to_nobody_key_work_lists() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let deputy = harness.member(&admin, "deputy@example.com").await;
    admin
        .set_role(deputy.account(), OrganizationRole::Administrator)
        .await
        .unwrap();
    let settings = admin
        .create_block(
            Uuid::now_v7(),
            SETTINGS_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .unwrap()
        .id;
    let secret = admin
        .create_block(
            Uuid::now_v7(),
            be_type_text::TextContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &be_block_content::BlockMetadata::default(),
            true,
        )
        .await
        .unwrap();
    let data = admin.current_key(secret.id).expect("the block has a key");
    let scope = admin.keys().scope_key(secret.id).expect("and a scope key");
    assert!(admin.keys().owner_only_covers(data) && admin.keys().owner_only_covers(scope));

    let mut work = admin.key_work().await.unwrap();
    for key in [scope, data] {
        work.unsealed.push(Unsealed {
            key,
            account: deputy.account(),
        });
    }
    admin.tend_with(work).await.unwrap();
    let sealed: i64 = harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys WHERE key_id IN (?1, ?2) AND wrapped_by = ?3",
            rusqlite::params![
                scope.to_string(),
                data.to_string(),
                format!("account:{}", deputy.account())
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(sealed, 0, "an owner-only block's key was sealed to another account");
}
