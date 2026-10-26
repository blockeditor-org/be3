use super::*;
use crate::folder::FolderContent;

#[test]
fn a_block_added_to_a_folder_by_both_sides_is_listed_once() {
    let block = Uuid::new_v4();
    let base = FolderContent::default();
    let ours = edited(&base, [base.root().add(block)]);
    let theirs = edited(&base, [base.root().add(block)]);

    let (merged, conflicts) = merged(&base, &ours, &theirs);
    let sequenced = edited(&base, [base.root().add(block), base.root().add(block)]);

    assert_eq!(conflicts, 0);
    for content in [&merged, &sequenced] {
        assert_eq!(content.root().blocks(), [block]);
        assert_eq!(content.references(), [block]);
        let removed = edited(content, [content.root().remove(block)]);
        assert!(removed.root().blocks().is_empty());
    }
}
