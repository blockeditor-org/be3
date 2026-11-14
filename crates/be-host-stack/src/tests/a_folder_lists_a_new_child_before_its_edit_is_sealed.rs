use super::*;
use be_type_workspace::FolderContent;

use be_block_content::ChildChange;

#[test]
fn a_folder_lists_a_new_child_before_its_edit_is_sealed() {
    let harness = Harness::start();
    harness.connect();
    let (folder, child) = (Uuid::new_v4(), Uuid::new_v4());
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    create(
        child,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    wait_until("knew the folder's type", |_| {
        is_known(FolderContent::CONTENT_TYPE)
    });
    assert_eq!(
        change_child(folder, FolderContent::CONTENT_TYPE, ChildChange::Add(child)),
        Some(true)
    );
    let unsealed = crate::wait_until(|shared| {
        shared
            .graph
            .get(folder)
            .filter(|node| node.references.contains(&child))
            .map(|_| shared.unsealed)
    });
    assert!(
        unsealed > 0,
        "the folder's graph node lists the child while its edit still waits for a seal"
    );
}
