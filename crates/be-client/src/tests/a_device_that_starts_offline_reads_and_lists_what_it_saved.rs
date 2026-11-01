use super::*;

use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn a_device_that_starts_offline_reads_and_lists_what_it_saved() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let path = harness.server.directory.join("device").join("refs.sqlite3");
    let store = MemoryStore::new();
    let (team, note, head) = {
        let refs = refs::RefsDb::open(&path).unwrap();
        let laptop = harness.device(&owner, &refs, &store).await;
        let team = workspace(&laptop, "Team").await;
        let note = laptop
            .create::<TextContent>(BlockParent::Block(team))
            .await
            .unwrap();
        let head = laptop
            .save(note, &TextBlock::of("written online"), None)
            .await
            .unwrap()
            .published()
            .unwrap();
        assert_eq!(laptop.children(BlockParent::Root).await.unwrap().len(), 1);
        assert_eq!(
            laptop
                .children(BlockParent::Block(team))
                .await
                .unwrap()
                .len(),
            1
        );
        (team, note, head)
    };

    let refs = refs::RefsDb::open(&path).unwrap();
    assert!(
        refs.knows(&harness.url, owner.account(), owner.organization())
            .unwrap()
    );
    let offline = harness.offline(&owner, &refs, &store);
    assert!(!offline.is_attached());
    assert_eq!(offline.role(), owner.role());
    assert_eq!(offline.kind(), owner.kind());

    let roots = offline.children(BlockParent::Root).await.unwrap();
    assert_eq!(
        roots.iter().map(|block| block.id).collect::<Vec<_>>(),
        [team]
    );
    let names = offline.metadata(&roots[0]).unwrap();
    assert_eq!(names, be_block::BlockMetadata::named("Team"));
    let inside = offline.children(BlockParent::Block(team)).await.unwrap();
    assert_eq!(
        inside.iter().map(|block| block.id).collect::<Vec<_>>(),
        [note]
    );
    assert_eq!(offline.local_head(note), Some(head));
    assert_eq!(offline.known_remote_head(note), Some(head));
    assert_eq!(text(&offline, note).await, "written online");
    assert!(matches!(
        offline.history(note).await,
        Err(ClientError::Disconnected(_))
    ));

    let manifest = offline
        .write_content(note, TextContent::CONTENT_TYPE, b"sealed with a cached key")
        .await
        .unwrap();
    assert!(!manifest.chunk_hashes().is_empty());

    let elsewhere = refs::RefsDb::open(&path).unwrap();
    assert!(
        Peer::cached(
            harness
                .config(&owner, &elsewhere)
                .organization(Some(Uuid::now_v7())),
            owner.account(),
            store.clone(),
        )
        .is_err(),
        "a workspace this device never opened came up from nothing"
    );

    harness.stop().await;
}
