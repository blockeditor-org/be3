use super::*;

fn listed_under(parent: Uuid, nodes: Vec<Node>, revision: u64) {
    with_shared_mut(|shared| {
        shared
            .graph
            .want(Owner::Editor(u64::MAX - 2), vec![Query::Children(parent)]);
        shared.graph.listed(
            crate::graph::Listing::Children(be_block_graph::BlockParent::Block(parent)),
            nodes,
            revision,
        );
    });
}

/// Offline, the worker works out what is out of reach as the graph moves, node by node: a block
/// this device holds is in reach, one that arrives in the graph with a head this device holds
/// no version of is out of reach at once, and one that leaves the graph is no longer listed as
/// out of reach.
#[test]
fn offline_a_block_is_out_of_reach_while_this_device_lacks_every_head() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 2);
    wait_for_count(block, 2);
    flush_and_send();
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    assert!(
        !out_of_reach(block),
        "offline, a block held whole is in reach"
    );

    let (parent, elsewhere) = (Uuid::new_v4(), Uuid::new_v4());
    let lacked = be_block_commit::CommitId::from_hash(be_block_store::Hash::of(b"elsewhere"));
    listed_under(
        parent,
        vec![Node {
            id: elsewhere,
            content_type: CounterContent::CONTENT_TYPE,
            author: None,
            parent: Some(be_block_graph::BlockParent::Block(parent)),
            access: be_block_graph::Access::Edit,
            references: Vec::new(),
            metadata: be_block_content::BlockMetadata::default(),
            head: Some(lacked),
            locked: false,
            held: true,
            sets_access: false,
            size: 0,
            tree_size: 0,
        }],
        u64::MAX - 1,
    );
    assert!(
        out_of_reach(elsewhere),
        "a block whose every head this device lacks is out of reach"
    );
    assert!(!out_of_reach(block), "the held block stays in reach");

    listed_under(parent, Vec::new(), u64::MAX);
    assert!(
        !out_of_reach(elsewhere),
        "a block that left the graph is not listed as out of reach"
    );
}
