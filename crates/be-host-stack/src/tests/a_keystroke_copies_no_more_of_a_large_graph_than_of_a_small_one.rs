use super::*;

/// What typing into a block copies of the worker's graph, which the page's `Seen` shares: the
/// nodes it changes, whatever the graph's size, not the whole graph per edit.
fn copied_by_a_keystroke(block: Uuid, count: &mut i64) -> u64 {
    let copied = || with_shared(|shared| shared.graph.copied_on_write).unwrap_or_default();
    let before = copied();
    *count += 1;
    add(block, 1);
    wait_for_count(block, *count);
    flush();
    copied().saturating_sub(before)
}

#[test]
fn a_keystroke_copies_no_more_of_a_large_graph_than_of_a_small_one() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    let mut count = 0;
    copied_by_a_keystroke(block, &mut count);
    let small = copied_by_a_keystroke(block, &mut count);

    // Twenty thousand more blocks, listed under one parent nothing else asks for.
    let parent = Uuid::new_v4();
    with_shared_mut(|shared| {
        let nodes: Vec<Node> = (0..20_000)
            .map(|_| Node {
                id: Uuid::new_v4(),
                content_type: Uuid::nil(),
                author: None,
                parent: Some(be_block_graph::BlockParent::Block(parent)),
                access: be_block_graph::Access::Edit,
                references: None,
                metadata: be_block_content::BlockMetadata::default(),
                head: None,
                locked: false,
                held: true,
                sets_access: false,
                size: 0,
                tree_size: 0,
                changed: 0,
                entry: None,
                unread: 0,
            })
            .collect();
        shared
            .graph
            .want(Owner::Editor(u64::MAX - 1), vec![Query::Children(parent)]);
        shared.graph.listed(
            crate::graph::Listing::Children(be_block_graph::BlockParent::Block(parent)),
            nodes,
            u64::MAX,
        );
    });
    assert!(with_graph(|graph| graph.nodes().count()).unwrap_or_default() > 20_000);

    let large = copied_by_a_keystroke(block, &mut count);
    assert!(
        large <= small + 16,
        "a keystroke copied {large} nodes of the large graph, {small} of the small one"
    );
}
