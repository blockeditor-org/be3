use super::*;
use workspace_ui_types::FolderContent;

use be_block::ChildChange;

fn listed(shared: &Shared, folder: Uuid) -> Vec<Uuid> {
    shared
        .graph
        .get(folder)
        .map(|node| node.contents())
        .unwrap_or_default()
}

#[test]
fn a_child_reordered_in_a_folder_is_listed_in_its_new_place() {
    let harness = Harness::start();
    harness.connect();
    let folder = Uuid::new_v4();
    let children = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    create(
        folder,
        FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    for child in children {
        create(
            child,
            CounterContent::CONTENT_TYPE,
            be_graph::BlockParent::Block(folder),
            be_block::BlockMetadata::default(),
            None,
        );
    }
    wait_until("knew the folder's type", |_| {
        is_known(FolderContent::CONTENT_TYPE)
    });
    for child in children {
        assert_eq!(
            change_child(folder, FolderContent::CONTENT_TYPE, ChildChange::Add(child)),
            Some(true)
        );
    }
    let [first, second, third] = children;
    wait_until("listed the children in the order they were added", |shared| {
        listed(shared, folder) == [first, second, third]
    });

    let reorder = |block, after| {
        change_child(
            folder,
            FolderContent::CONTENT_TYPE,
            ChildChange::Reorder { block, after },
        )
    };
    assert_eq!(reorder(third, None), Some(true));
    wait_until("listed the last child first", |shared| {
        listed(shared, folder) == [third, first, second]
    });
    assert_eq!(reorder(third, Some(second)), Some(true));
    wait_until("listed the child after the one it was put after", |shared| {
        listed(shared, folder) == [first, second, third]
    });
}
