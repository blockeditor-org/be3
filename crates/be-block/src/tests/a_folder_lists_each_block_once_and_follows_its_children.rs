use super::*;
use crate::folder::{Folder, FolderContent};
use crate::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_folder_lists_each_block_once_and_follows_its_children() {
    let (first, second, copy) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let folder = edited(&FolderContent::default(), [Folder::default().add(first)]);
    let folder = edited(
        &folder,
        [folder.root().add(second), folder.root().add(first)],
    );
    assert_eq!(folder.root().blocks(), vec![first, second]);

    let replaced = edited(
        &folder,
        folder.root().child_edit(ChildChange::Replace {
            old: first,
            new: copy,
        }),
    );
    assert_eq!(replaced.root().blocks(), vec![copy, second]);

    let merged = edited(
        &replaced,
        replaced.root().child_edit(ChildChange::Replace {
            old: copy,
            new: second,
        }),
    );
    assert_eq!(merged.root().blocks(), vec![second]);

    let deleted = edited(
        &merged,
        merged.root().child_edit(ChildChange::Delete(second)),
    );
    assert!(deleted.root().blocks().is_empty());
}
