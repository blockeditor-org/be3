use super::*;
use workspace_ui_types::FolderContent;

use be_block::{DELETED_FROM, DeletedFrom};
use be_graph::BlockParent;

#[test]
fn a_deleted_block_remembers_where_it_was_until_it_is_restored() {
    let harness = Harness::start();
    harness.connect();
    let (folder, note) = (Uuid::new_v4(), Uuid::new_v4());
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    create(
        note,
        CounterContent::CONTENT_TYPE,
        BlockParent::Block(folder),
        be_block::BlockMetadata::default(),
        None,
    );
    let remembered = || node(note).and_then(|node| node.metadata.get(&DELETED_FROM));
    let deleted = || with_graph(|graph| graph.deleted_root(note)).flatten();
    assert_eq!(remembered(), None);

    set_parent(note, BlockParent::Detached);
    assert_eq!(remembered(), Some(DeletedFrom::Block(folder)));
    assert_eq!(deleted(), Some(note));

    set_parent(note, BlockParent::Detached);
    assert_eq!(
        remembered(),
        Some(DeletedFrom::Block(folder)),
        "deleting it again keeps where it first was"
    );

    set_parent(note, BlockParent::Block(folder));
    assert_eq!(
        remembered(),
        None,
        "a restored block forgets it was deleted"
    );
    assert_eq!(deleted(), None);
}
