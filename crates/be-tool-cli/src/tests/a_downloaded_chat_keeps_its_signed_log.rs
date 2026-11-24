use super::*;

/// `be download` writes a chat's signed log into the block file, each entry as its author
/// signed it, as be-host's export does: both are `be_block_type_host::transfer::collect`.
#[tokio::test]
async fn a_downloaded_chat_keeps_its_signed_log() {
    let harness = Harness::start().await;
    let owner = harness.owner().await;
    let parent = owner.children(BlockParent::Root).await.unwrap()[0].id;
    let chat = Uuid::now_v7();
    owner
        .create_block(
            chat,
            be_type_chat::ChatContent::CONTENT_TYPE,
            BlockParent::Block(parent),
            &BlockMetadata::named("Talk"),
            false,
        )
        .await
        .unwrap();
    for said in ["one", "two"] {
        owner
            .append(
                chat,
                "be3.chat",
                said.as_bytes(),
                be_block_content::Along::default(),
            )
            .await
            .unwrap();
    }
    let tracker = Tracker::new(owner, sources());

    std::fs::create_dir_all(&harness.directory).unwrap();
    let file = harness.directory.join("talk.block");
    let file = file.to_str().unwrap().to_owned();
    run(&tracker, &format!("Talk download {file}"))
        .await
        .unwrap();
    let saved = be_block_client::transfer::decode(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(saved[0].id, chat);
    assert_eq!(
        saved[0]
            .log
            .iter()
            .map(|entry| entry.said.as_ref().map(|said| said.bytes.clone()))
            .collect::<Vec<_>>(),
        [Some(b"one".to_vec()), Some(b"two".to_vec())],
        "the block file carries the chat's log"
    );
    harness.stop().await;
}
