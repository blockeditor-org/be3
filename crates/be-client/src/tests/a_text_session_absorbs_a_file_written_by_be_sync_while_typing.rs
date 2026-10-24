use super::*;

use be_block::{TextBlock, TextContent, plain};

#[tokio::test]
async fn a_text_session_absorbs_a_file_written_by_be_sync_while_typing() {
    let harness = Harness::start().await;
    let author = harness.owner("typist@example.com").await;
    let block = author
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();
    author
        .save(
            block,
            &TextBlock::of("first line\nmiddle line\nlast line\n"),
            None,
        )
        .await
        .unwrap()
        .published()
        .unwrap();
    let author = Arc::new(author);
    let sync = harness.shared(&author).await;
    let mut session = Live::<_, TextContent>::join(Arc::clone(&author), block)
        .await
        .unwrap();
    assert!(session.is_owner());
    type_at(&mut session, 0, "typed ").await;

    let read = sync.open::<TextContent>(block).await.unwrap().unwrap();
    let head = sync.local_head(block);
    let file = plain::update_block(
        TextContent::CONTENT_TYPE,
        Some(&read.encode()),
        "notes.txt",
        b"first line\nmiddle line\nlast line, from disk\n".to_vec(),
        7,
        sync.organization(),
    )
    .unwrap();
    let published = sync
        .save_encoded(
            block,
            file.block_type,
            &file.content,
            file.references,
            file.derived,
            head,
            None,
        )
        .await
        .unwrap()
        .published()
        .expect("be sync publishes against the head it read");

    session.published_elsewhere(published).await.unwrap();
    let expected = "typed first line\nmiddle line\nlast line, from disk\n";
    assert_eq!(
        session.content().to_text(),
        expected,
        "the session keeps what was typed and takes in what came from disk"
    );
    assert!(session.is_clean());
    assert_eq!(
        sync.open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .to_text(),
        expected,
        "the merge is published for everyone else"
    );
    harness.stop().await;
}
