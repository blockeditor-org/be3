use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_workspace_not_kept_for_offline_use_is_forgotten_but_its_unsent_edits_stay() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let path = harness.directory.join("device").join("refs.sqlite3");
    let store = MemoryStore::new();
    let (kept, dropped, note, other, waiting) = {
        let refs = refs::RefsDb::open(&path).unwrap();
        let laptop = harness.device(&owner, &refs, &store).await;
        let kept = workspace(&laptop, "Kept").await;
        let dropped = workspace(&laptop, "Not kept").await;
        let note = laptop
            .create::<TextContent>(BlockParent::Block(kept))
            .await
            .unwrap();
        laptop
            .save(note, &TextBlock::of("kept"), None)
            .await
            .unwrap();
        let other = laptop
            .create::<TextContent>(BlockParent::Block(dropped))
            .await
            .unwrap();
        laptop
            .save(other, &TextBlock::of("only for this session"), None)
            .await
            .unwrap();
        let waiting = laptop
            .create::<TextContent>(BlockParent::Block(dropped))
            .await
            .unwrap();
        assert_eq!(text(&laptop, note).await, "kept");
        assert_eq!(text(&laptop, other).await, "only for this session");
        laptop.children(BlockParent::Block(kept)).await.unwrap();
        laptop.children(BlockParent::Block(dropped)).await.unwrap();
        (kept, dropped, note, other, waiting)
    };

    let refs = refs::RefsDb::open(&path).unwrap();
    let offline = harness.offline(&owner, &refs, &store);
    let _ = offline
        .save(waiting, &TextBlock::of("written offline"), None)
        .await;
    assert!(offline.has_pending(waiting));
    assert_eq!(
        offline
            .forget_offline(|workspace| workspace != dropped)
            .unwrap(),
        1
    );
    assert_eq!(text(&offline, note).await, "kept");
    assert!(
        offline
            .children(BlockParent::Block(dropped))
            .await
            .unwrap()
            .iter()
            .all(|block| block.id != other),
        "a workspace not kept offline still lists what it held"
    );
    assert!(
        offline.open::<TextContent>(other).await.is_err(),
        "a block of a workspace not kept offline still opened offline"
    );
    assert!(
        offline.local_head(waiting).is_some(),
        "an edit that has not reached the server was forgotten"
    );
    let _ = kept;

    harness.stop().await;
}
