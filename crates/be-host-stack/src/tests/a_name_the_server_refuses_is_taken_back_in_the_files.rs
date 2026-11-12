use super::*;

#[test]
fn a_name_the_server_refuses_is_taken_back_in_the_files() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Plan"),
        None,
        false,
    );
    flush_and_send();

    let refused = "n".repeat(70 * 1024);
    set_name(block, Some(refused.clone()));
    assert_eq!(
        node(block).and_then(|node| node.metadata.name()),
        Some(refused),
        "a new name shows at once"
    );
    wait_until("took the refused name back", |shared| {
        shared
            .graph
            .get(block)
            .and_then(|node| node.metadata.name())
            .as_deref()
            == Some("Plan")
    });
}
