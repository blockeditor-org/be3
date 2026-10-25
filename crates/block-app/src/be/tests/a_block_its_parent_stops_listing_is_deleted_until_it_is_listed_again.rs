use super::*;
use workspace_ui_types::FolderContent;

use be_block::ChildChange;
use be_graph::BlockParent;

#[test]
fn a_block_its_parent_stops_listing_is_deleted_until_it_is_listed_again() {
    let harness = Harness::start();
    harness.connect();
    let (folder, note) = (Uuid::new_v4(), Uuid::new_v4());
    ask_for(vec![Query::Roots, Query::Children(folder)]);
    let mut listing = FolderContent::default();
    listing.apply(&workspace_ui_types::Folder::default().add(note));
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Root,
        be_block::BlockMetadata::default(),
        Some(listing.encode()),
    );
    create(
        note,
        CounterContent::CONTENT_TYPE,
        BlockParent::Block(folder),
        be_block::BlockMetadata::default(),
        None,
    );
    hold(folder, FolderContent::CONTENT_TYPE);
    let deleted = |shared: &Shared| shared.graph.deleted_root(note);
    wait_until("listed the note", |shared| {
        shared.blocks.contains_key(&folder)
            && shared.graph.get(note).is_some()
            && deleted(shared).is_none()
    });

    assert_eq!(
        change_child(
            folder,
            FolderContent::CONTENT_TYPE,
            ChildChange::Delete(note)
        ),
        Some(true)
    );
    wait_until("took the note out of the folder", |shared| {
        deleted(shared) == Some(note)
    });
    assert_eq!(
        node(note).and_then(|node| node.parent),
        Some(BlockParent::Block(folder)),
        "deleting it leaves its parent as it was"
    );

    assert_eq!(
        change_child(folder, FolderContent::CONTENT_TYPE, ChildChange::Add(note)),
        Some(true)
    );
    wait_until("listed the note again", |shared| deleted(shared).is_none());
}
