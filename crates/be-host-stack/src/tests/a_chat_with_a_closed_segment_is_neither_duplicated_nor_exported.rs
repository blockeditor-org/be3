use be_block::{BlockContent, LiveEdit};
use be_block_client::Write;
use be_type_chat::{ChatContent, Message};

use super::*;

#[test]
fn a_chat_with_a_closed_segment_is_neither_duplicated_nor_exported() {
    let harness = Harness::start();
    let block = Uuid::new_v4();

    // A chat whose oldest messages are closed into a segment: parts after the header.
    let author = Uuid::new_v4();
    let body = "a line of chat ".repeat(70);
    let mut chat = ChatContent::default();
    for index in 0..120 {
        let (_, send) = chat.send(block, Message::new(author, index, body.clone()));
        chat.apply(&send.into());
    }
    let closing = chat.closing().expect("a chat this long closes a segment");
    chat.apply(&closing);
    assert_eq!(chat.segments().len(), 1);

    let runtime = runtime();
    let local = tokio::task::LocalSet::new();
    let config = harness.outside_config();
    local.block_on(&runtime, async move {
        let peer = outside_peer(config).await;
        peer.ensure::<ChatContent>(block, be_block_graph::BlockParent::Root)
            .await
            .expect("the chat is created");
        let head = peer.newest_head(block).await.expect("the head is read");
        peer.write(
            block,
            Write::encoded(ChatContent::CONTENT_TYPE, chat.parts(), head),
        )
        .await
        .expect("the chat saves with its segment");
    });

    harness.connect();
    ask_for(vec![Query::Block(block)]);
    wait_until("heard the chat join the graph", |shared| {
        shared.graph.get(block).is_some()
    });
    let (copy, _) = duplicate(block).expect("the chat is in the graph");
    wait_until("the copy was refused", |shared| {
        shared
            .error
            .as_deref()
            .is_some_and(|error| error.contains("keeps parts after its first"))
    });
    assert!(
        node(copy).is_none_or(|copy| copy.head.is_none()),
        "the copy holds no content: its closed history would be lost"
    );

    let (sender, receiver) = std::sync::mpsc::channel();
    export(block, move |exported| {
        let _ = sender.send(exported);
    });
    let exported = receiver.recv().expect("the export answers");
    assert!(
        exported.is_err_and(|error| error.contains("not exported yet")),
        "a block file would carry only the header, so it is refused"
    );
}
