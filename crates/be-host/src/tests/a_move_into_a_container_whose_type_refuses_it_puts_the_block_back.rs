use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};
use be_type_workspace::{Folder, FolderContent};

use super::*;

/// A text takes no child at its type: a block moved into one from a folder, which the folder let
/// go of, goes back into the folder, and the person is told why.
#[test]
fn a_move_into_a_container_whose_type_refuses_it_puts_the_block_back() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (folder, child, notes) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    let mut listing = FolderContent::default();
    listing.apply(&Folder::default().add(child));
    make(folder, FolderContent::CONTENT_TYPE, BlockParent::Root, "Folder", listing.encode());
    make(child, text, BlockParent::Block(folder), "Child", TextBlock::of("child").encode());
    make(notes, text, BlockParent::Root, "Notes", TextBlock::of("notes").encode());
    synced();
    known(&[text, FolderContent::CONTENT_TYPE]);
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
    be_host_stack::wait_until("the folder lists the child again", |shared| {
        shared
            .graph
            .get(folder)
            .is_some_and(|node| node.references.contains(&child))
            .then_some(())
    });
    assert_eq!(
        be_host_stack::node(child).and_then(|node| node.parent),
        Some(BlockParent::Block(folder)),
        "the child is still the folder's"
    );
}
