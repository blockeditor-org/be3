use super::*;

use workspace_ui_types::FolderContent;

fn exported(block: Uuid) -> Vec<u8> {
    let (sender, receiver) = std::sync::mpsc::channel();
    export(block, move |exported| {
        let _ = sender.send(exported);
    });
    receiver
        .recv_timeout(PATIENCE)
        .expect("the export answers")
        .expect("the export succeeds")
        .1
}

fn folder(parent: be_graph::BlockParent) -> Uuid {
    let block = Uuid::new_v4();
    create(
        block,
        FolderContent::CONTENT_TYPE,
        parent,
        be_block::BlockMetadata::default(),
        None,
    );
    block
}

#[test]
fn an_imported_tree_shows_at_once_and_moves_once_it_is_uploaded() {
    let harness = Harness::start();
    harness.connect();
    let top = folder(be_graph::BlockParent::Root);
    let mut counters = Vec::new();
    for count in 1..=12 {
        let inner = folder(be_graph::BlockParent::Block(top));
        let counter = Uuid::new_v4();
        create(
            counter,
            CounterContent::CONTENT_TYPE,
            be_graph::BlockParent::Block(inner),
            be_block::BlockMetadata::default(),
            None,
        );
        open(counter, CounterContent::CONTENT_TYPE);
        add(counter, count);
        wait_for_count(counter, count);
        counters.push(counter);
    }
    let bytes = exported(top);
    let destination = folder(be_graph::BlockParent::Root);

    let (copy, _) = import(&bytes, be_graph::BlockParent::Detached).expect("the file imports");
    ask_for(vec![Query::Subtree(copy)]);
    let inner = query(Query::Children(copy));
    let deepest = query(Query::Children(inner[0].id))[0].id;
    open(deepest, CounterContent::CONTENT_TYPE);
    assert!(
        content(deepest).is_some(),
        "an imported block shows what it holds before the server has it"
    );
    set_parent(copy, be_graph::BlockParent::Block(destination));

    let mut counts: Vec<i64> = inner
        .iter()
        .map(|inner| {
            let counter = query(Query::Children(inner.id))[0].id;
            open(counter, CounterContent::CONTENT_TYPE);
            wait_until("the copy's session joined", |shared| {
                shared.blocks.contains_key(&counter) && !shared.previewed.contains(&counter)
            });
            count_of(counter).expect("every copied counter is readable after the move")
        })
        .collect();
    counts.sort_unstable();
    assert_eq!(counts, (1..=12).collect::<Vec<i64>>());
    assert_eq!(
        node(copy).and_then(|node| node.parent),
        Some(be_graph::BlockParent::Block(destination))
    );
    assert!(status().error.is_none(), "{:?}", status().error);
}
