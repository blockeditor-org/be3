use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::FolderContent;

use super::*;

/// A text takes no child at its type: a block moved into one from a folder, which the folder let
/// go of, goes back into the folder where it sat, and the person is told why.
#[test]
fn a_move_into_a_container_whose_type_refuses_it_puts_the_block_back() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (folder, child, notes) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let (first, last) = (Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    let mut listing = FolderContent::default();
    for held in [first, child, last] {
        listing.apply(&listing.root().add(held));
    }
    make(folder, FolderContent::CONTENT_TYPE, BlockParent::Root, "Folder", listing.encode());
    for (held, name) in [(first, "First"), (child, "Child"), (last, "Last")] {
        make(held, text, BlockParent::Block(folder), name, TextBlock::of(name).encode());
    }
    make(notes, text, BlockParent::Root, "Notes", TextBlock::of("notes").encode());
    synced();
    let _apps = known(&[text, FolderContent::CONTENT_TYPE]);
    // As the file tree does, which lists the folder, so the child's name is known.
    be_host_stack::want(
        be_host_stack::Owner::Editor(1),
        vec![
            be_host_stack::Query::Roots,
            be_host_stack::Query::Parents(folder),
            be_host_stack::Query::Subtree(folder),
        ],
    );
    be_host_stack::wait_until("listed the folder", |shared| {
        shared.graph.get(child).map(|_| ())
    });
    let mut core = core();
    core.block_types.insert(folder, FolderContent::CONTENT_TYPE);
    core.block_types.insert(child, text);
    core.block_types.insert(notes, text);

    core.queue_move(child, text, SidebarDragSource::Block(folder), notes, false);
    settle(&mut core);

    assert!(refused("Notes can't hold Child"));
    be_host_stack::wait_until("the folder holds the child where it sat", |shared| {
        let held = shared.blocks.get(&folder)?;
        let content = FolderContent::decode(held.bytes()).ok()?;
        let order: Vec<Uuid> = content.root().entries.iter().map(|entry| entry.block.0).collect();
        (order == [first, child, last]).then_some(())
    });
    assert_eq!(
        be_host_stack::node(child).and_then(|node| node.parent),
        Some(BlockParent::Block(folder)),
        "the child is still the folder's"
    );
}
