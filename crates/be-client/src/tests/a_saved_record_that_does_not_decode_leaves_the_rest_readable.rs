use text_block_types::{TextBlock, TextContent};

use super::*;

#[tokio::test]
async fn a_saved_record_that_does_not_decode_leaves_the_rest_readable() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let path = std::env::temp_dir().join(format!("be-client-refs-{}.sqlite3", Uuid::new_v4()));
    let store = MemoryStore::new();
    let note = {
        let refs = refs::RefsDb::open(&path).unwrap();
        let laptop = harness.device(&owner, &refs, &store).await;
        let team = workspace(&laptop, "Team").await;
        let note = laptop
            .create::<TextContent>(BlockParent::Block(team))
            .await
            .unwrap();
        laptop
            .save(note, &TextBlock::of("kept"), None)
            .await
            .unwrap();
        note
    };
    let damaged = rusqlite::Connection::open(&path).unwrap();
    damaged
        .execute(
            "INSERT INTO refs (server, account, organization, kind, key, value)
             SELECT server, account, organization, kind, X'00112233445566778899aabbccddeeff', X'01ff'
             FROM refs WHERE kind = 2 LIMIT 1",
            [],
        )
        .unwrap();
    drop(damaged);

    let refs = refs::RefsDb::open(&path).unwrap();
    let offline = harness.offline(&owner, &refs, &store);
    assert!(offline.local_head(note).is_some());
    assert_eq!(text(&offline, note).await, "kept");

    let _ = std::fs::remove_file(&path);
    harness.stop().await;
}
