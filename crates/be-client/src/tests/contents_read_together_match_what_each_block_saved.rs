use super::*;
use text_block_types::{TextBlock, TextContent};

#[tokio::test]
async fn contents_read_together_match_what_each_block_saved() {
    let harness = Harness::start().await;
    let author = harness.owner("archivist@example.com").await;
    let workspace = workspace(&author, "Archive").await;
    let mut wanted = vec![(workspace, None)];
    let mut expected = vec![None];
    for index in 0..300 {
        let note = author
            .create::<TextContent>(BlockParent::Block(workspace))
            .await
            .unwrap();
        let written = TextBlock::of(&format!("note {index}"));
        author.save(note, &written, None).await.unwrap();
        wanted.push((note, None));
        expected.push(Some(written.encode()));
    }
    let photo = author
        .create::<ImageContent>(BlockParent::Block(workspace))
        .await
        .unwrap();
    let large = image("large.png", be_protocol::MAX_FRAME_BYTES, 9);
    author.save(photo, &large, None).await.unwrap();
    wanted.push((photo, None));
    expected.push(Some(large.encode()));

    let reader = harness.second(&author).await;
    let read = reader.read_contents(&wanted).await.unwrap();
    let ids: Vec<Uuid> = read.iter().map(|(summary, _)| summary.id).collect();
    assert_eq!(ids, wanted.iter().map(|(id, _)| *id).collect::<Vec<_>>());
    let contents: Vec<Option<Vec<u8>>> = read.into_iter().map(|(_, content)| content).collect();
    assert!(
        contents == expected,
        "a content read differs from what was saved"
    );

    let heads: Vec<_> = reader
        .read_contents(&wanted)
        .await
        .unwrap()
        .into_iter()
        .map(|(summary, content)| {
            assert!(content.is_some() || summary.id == workspace);
            (summary.id, summary.head)
        })
        .collect();
    let again = reader.read_contents(&heads).await.unwrap();
    assert!(
        again.into_iter().map(|(_, content)| content).eq(expected),
        "contents the reader already held read differently"
    );

    harness.stop().await;
}
