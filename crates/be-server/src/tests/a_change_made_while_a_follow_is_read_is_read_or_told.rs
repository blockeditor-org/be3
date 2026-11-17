use super::*;

#[tokio::test]
async fn a_change_made_while_a_follow_is_read_is_read_or_told() {
    let harness = Harness::start().await;
    let mut writer = harness.client().await;
    writer.register("author@example.com").await;
    let organization = writer.organization("notes").await;
    let folder = writer.create_block(BlockParent::Root).await;
    for _ in 0..3 {
        writer.create_block(BlockParent::Block(folder)).await;
    }
    let mut reader = harness
        .second_connection(&writer.token.clone(), organization)
        .await;

    // The first page follows the tree; a change lands before the rest are read.
    let start = ChangeCursor {
        changed: 0,
        block: Uuid::max(),
    };
    let page = reader
        .call(|peer| client::blocks::changed_since(peer, folder, start, 1, true))
        .await
        .expect("the first page");
    let mut after = page.next.expect("the tree takes more than a page");
    let late = writer.create_block(BlockParent::Block(folder)).await;
    let mut read = Vec::new();
    loop {
        let page = reader
            .call(|peer| client::blocks::changed_since(peer, folder, after, 1, false))
            .await
            .expect("a later page");
        read.extend(page.summaries.iter().map(|summary| summary.id));
        match page.next {
            Some(next) => after = next,
            None => {
                reader.cursors.insert(
                    folder,
                    ChangeCursor {
                        changed: page.sequence,
                        block: Uuid::max(),
                    },
                );
                break;
            }
        }
    }
    assert!(read.contains(&late), "a change during the read is in it");

    // Read to its end, the tree is told of the next change.
    let next = writer.create_block(BlockParent::Block(folder)).await;
    let told = reader.heard_change(next, |_| true).await;
    assert_eq!(told.id, next);

    harness.stop().await;
}
