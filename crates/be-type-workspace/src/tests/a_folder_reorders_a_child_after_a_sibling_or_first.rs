use super::*;
use crate::folder::{Folder, FolderContent};
use be_block::{ChildChange, Root};
use be_model::Edit;
use uuid::Uuid;

#[test]
fn a_folder_reorders_a_child_after_a_sibling_or_first() {
    let (first, second, third) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let folder = edited(&FolderContent::default(), [Folder::default().add(first)]);
    let folder = edited(&folder, [folder.root().add(second)]);
    let folder = edited(&folder, [folder.root().add(third)]);
    assert_eq!(folder.references(), vec![first, second, third]);

    let reorder = |folder: &FolderContent, block, after| {
        edited(
            folder,
            folder
                .root()
                .child_edit(ChildChange::Reorder { block, after }),
        )
    };
    let moved = reorder(&folder, third, Some(first));
    assert_eq!(moved.root().blocks(), vec![first, third, second]);
    assert_eq!(
        moved.references(),
        vec![first, third, second],
        "the references the file tree lists follow the new order"
    );

    let moved = reorder(&moved, second, None);
    assert_eq!(moved.root().blocks(), vec![second, first, third]);
    assert_eq!(moved.references(), vec![second, first, third]);

    let unknown = Uuid::new_v4();
    for (block, after) in [
        (unknown, None),
        (first, Some(unknown)),
        (first, Some(first)),
        (first, Some(second)),
        (second, None),
    ] {
        assert_eq!(
            moved
                .root()
                .child_edit(ChildChange::Reorder { block, after }),
            Some(Edit::default()),
            "reordering {block} after {after:?} changes nothing"
        );
    }
}
