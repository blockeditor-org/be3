use super::*;

/// A block file opens for anyone, so `be download` refuses a block only its owner can read, as
/// be-host's export does.
#[tokio::test]
async fn a_block_only_its_owner_reads_is_not_downloaded() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let settings = Uuid::now_v7();
    owner
        .create_block(
            settings,
            be_block_graph::SETTINGS_TYPE,
            BlockParent::Root,
            &BlockMetadata::default(),
            false,
        )
        .await
        .unwrap();
    let diary = Uuid::now_v7();
    owner
        .create_block(
            diary,
            be_type_text::TextContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &BlockMetadata::named("Diary"),
            true,
        )
        .await
        .unwrap();
    let tracker = Tracker::new(owner, sources());

    std::fs::create_dir_all(&harness.directory).unwrap();
    let file = harness.directory.join("diary.block");
    let file = file.to_str().unwrap().to_owned();
    let refused = run(&tracker, &format!("{diary} download {file}"))
        .await
        .unwrap_err();
    assert!(refused.contains("only you can read"), "{refused}");
    assert!(!std::path::Path::new(&file).exists());
    harness.stop().await;
}
