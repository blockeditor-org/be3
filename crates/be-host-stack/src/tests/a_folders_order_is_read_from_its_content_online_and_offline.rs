use super::*;
use be_block_graph::BlockParent;
use be_type_text::TextContent;
use be_type_workspace::FolderContent;

/// The ids of what the mirror answers `Query::References(folder)`, in its order.
fn order(shared: &Shared, folder: Uuid) -> Vec<Uuid> {
    shared
        .graph
        .query(Query::References(folder))
        .iter()
        .map(|node| node.id)
        .collect()
}

/// No summary names what a block references (the server keeps them only for liveness, knowing
/// a block exists and backrefs): a folder shown with its children in order, which this device
/// never opens a replica of, is read from its content, and offline from the content it kept.
#[test]
fn a_folders_order_is_read_from_its_content_online_and_offline() {
    let mut harness = Harness::start();
    let other = Device::online(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let (folder, order_made) = other.runtime.block_on(async {
        let folder = peer
            .create::<FolderContent>(BlockParent::Root)
            .await
            .expect("the other device makes a folder");
        let mut children = Vec::new();
        for _ in 0..3 {
            children.push(
                peer.create::<TextContent>(BlockParent::Block(folder))
                    .await
                    .expect("the other device makes a child"),
            );
        }
        // In an order of their own, not of their ids, which is how the server lists them.
        children.sort();
        let order = vec![children[2], children[0], children[1]];
        let mut content = FolderContent::default();
        for child in &order {
            let add = content.root().add(*child);
            content.apply(&add);
        }
        peer.save(folder, &content, None)
            .await
            .expect("the other device saves the folder");
        peer.drain(|_| false)
            .await
            .expect("the other device sends it all");
        (folder, order)
    });

    harness.connect();
    ask_for(vec![Query::References(folder)]);
    wait_until("read the folder's order from its content", |shared| {
        shared.graph.ready(Query::References(folder)) && order(shared, folder).len() == 3
    });
    let (read, opened) = with_shared(|shared| {
        (order(shared, folder), shared.blocks.contains_key(&folder))
    })
    .expect("the stack runs");
    assert_eq!(read, order_made, "the folder's order is its content's");
    assert!(!opened, "read without opening a replica of the folder");

    // Offline, after a restart: the folder's content this device kept still orders it.
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    stop();
    harness.connect();
    ask_for(vec![Query::References(folder)]);
    wait_until("read the folder's order offline", |shared| {
        shared.graph.ready(Query::References(folder)) && order(shared, folder).len() == 3
    });
    assert!(!status().connected);
    assert_eq!(
        with_shared(|shared| order(shared, folder)).expect("the stack runs"),
        order_made,
        "offline, the folder's order is the content this device kept"
    );
}
