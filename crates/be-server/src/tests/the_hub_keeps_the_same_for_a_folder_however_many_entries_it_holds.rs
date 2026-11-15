use be_block_graph::{Access, BlockGraph, BlockNode};

use crate::watch::{Facts, Push, Seen, WatchHub};

use super::*;

const FOLDER: Uuid = Uuid::from_u128(0xf01d);

/// A folder of `entries` blocks, each in the graph.
fn folder(graph: &mut BlockGraph, entries: usize) -> Vec<Uuid> {
    let author = Uuid::from_u128(1);
    graph
        .insert(
            FOLDER,
            BlockNode::new(Uuid::nil(), author, BlockParent::Root),
        )
        .unwrap();
    let entries: Vec<Uuid> = (0..entries).map(|_| Uuid::new_v4()).collect();
    for entry in &entries {
        graph
            .insert(
                *entry,
                BlockNode::new(Uuid::nil(), author, BlockParent::Root),
            )
            .unwrap();
    }
    graph.set_references(FOLDER, &entries).unwrap();
    entries
}

/// A summary the caller may view of `block`, as the graph has it.
fn viewed(graph: &BlockGraph, block: Uuid) -> BlockSummary {
    BlockSummary {
        parent: Some(BlockParent::Root),
        references: graph.references(block),
        ..BlockSummary::no_access(block, Uuid::nil(), Access::View)
    }
}

/// The bytes the hub keeps for a connection that read the folder's summary, and so was told
/// every one of its references.
async fn kept_reading(graph: &BlockGraph) -> usize {
    let hub = WatchHub::new();
    let (sender, _receiving) = tokio::sync::mpsc::channel(16);
    let client = hub.register(sender).await;
    let listing = Listing::Block(FOLDER);
    hub.list(client, listing).await.unwrap();
    let told = Facts::of(&viewed(graph, FOLDER), Seen::default());
    hub.listed(client, listing, vec![(FOLDER, told)]).await;
    hub.bytes_kept(client).await
}

/// The hub keeps what it told a connection of a block that the graph cannot answer, not the
/// block's references: a connection that read a folder of `MAX_REFERENCES` entries costs it
/// what one that read a folder of one does. Which listings hold a changed block through
/// references it reads from the graph as the change left it.
#[tokio::test]
async fn the_hub_keeps_the_same_for_a_folder_however_many_entries_it_holds() {
    let mut small = BlockGraph::new();
    folder(&mut small, 1);
    let mut large = BlockGraph::new();
    let entries = folder(&mut large, be_block_protocol::MAX_REFERENCES);
    assert_eq!(kept_reading(&small).await, kept_reading(&large).await);

    let hub = WatchHub::new();
    let (sender, mut receiving) = tokio::sync::mpsc::channel(16);
    let client = hub.register(sender).await;
    hub.list(client, Listing::References(FOLDER)).await.unwrap();
    let links = hub.links(&[client]).await.remove(0);
    let stranger = Uuid::new_v4();
    large
        .insert(
            stranger,
            BlockNode::new(Uuid::nil(), Uuid::from_u128(1), BlockParent::Root),
        )
        .unwrap();
    for (block, linked) in [(stranger, false), (entries[1234], true)] {
        let seen = Seen {
            entry: false,
            linked: links.hold(&large, block, |_| true),
        };
        assert_eq!(seen.linked, linked);
        let change = Push::BlockChanged {
            block: viewed(&large, block),
            moved: None,
            from: 7,
        };
        hub.tell(client, change, seen, &links).await;
    }
    match receiving.try_recv() {
        Ok(Push::BlockChanged { block, .. }) => assert_eq!(block.id, entries[1234]),
        other => panic!("the folder's entry was not told alone: {other:?}"),
    }
    assert!(receiving.try_recv().is_err(), "the stranger was told too");
}
