use be_block_client::Write;
use be_block_content::BlockContent;
use be_type_workspace::{FileContent, FileHeader};

use super::*;

#[test]
fn a_file_still_duplicates_and_exports_though_it_saves_in_parts() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    let file = FileContent::new(
        FileHeader {
            source_name: "notes.txt".to_owned(),
            media_type: "text/plain".to_owned(),
        },
        "hello world".repeat(100).into_bytes(),
    );

    let runtime = runtime();
    let local = tokio::task::LocalSet::new();
    let config = harness.outside_config();
    local.block_on(&runtime, async move {
        let peer = outside_peer(config).await;
        peer.ensure::<FileContent>(block, be_block_graph::BlockParent::Root)
            .await
            .expect("the file is created");
        let head = peer.newest_head(block).await.expect("the head is read");
        peer.write(
            block,
            Write::encoded(FileContent::CONTENT_TYPE, file.parts(), head),
        )
        .await
        .expect("the file saves");
    });

    harness.connect();
    ask_for(vec![Query::Block(block)]);
    wait_until("heard the file join the graph", |shared| {
        shared.graph.get(block).is_some()
    });
    // A file's `encode` is all of it, so a copy through bytes loses nothing and is not refused.
    let (copy, _) = duplicate(block).expect("the file is in the graph");
    ask_for(vec![Query::Block(copy)]);
    wait_until("the copy holds the file", |shared| {
        shared
            .graph
            .get(copy)
            .is_some_and(|node| node.head.is_some())
    });
    let (sender, receiver) = std::sync::mpsc::channel();
    export(block, move |exported| {
        let _ = sender.send(exported);
    });
    receiver
        .recv()
        .expect("the export answers")
        .expect("a file exports");
}
