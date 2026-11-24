use super::*;

/// A search unfolds everything, whatever is open, and keeps only the blocks it matches and those
/// above them, with no notes.
#[test]
fn a_search_keeps_what_matches_and_the_blocks_above_it() {
    let mut deleted = block(5, 1, &[]);
    deleted.deleted = true;
    let blocks = [
        block(1, 9, &[2, 4, 5]),
        block(2, 1, &[3]),
        block(3, 2, &[]),
        block(4, 1, &[]),
        deleted,
        block(6, 9, &[]),
    ];
    let graph = graph(&blocks);
    let mut rows = Vec::new();
    let kept = BlockTree {
        children: &mut |row| children_of(&graph, &row.block),
        open: &|_| false,
        shown: &|_| true,
        matches: Some(&|block| [3, 5].contains(&block.id.as_u128())),
    }
    .walk(
        vec![blocks[0].clone(), blocks[5].clone()],
        &[],
        0,
        false,
        &mut rows,
    );
    assert!(kept);
    assert_eq!(paths(&rows), [vec![1], vec![1, 2], vec![1, 2, 3], vec![1, 5]]);
    let TreeEntry::Block(deleted) = &rows[3] else {
        panic!("a block row");
    };
    assert!(!deleted.in_deleted);
    assert!(rows.iter().all(|row| match row {
        TreeEntry::Block(row) => row.expanded,
        TreeEntry::Note { .. } => false,
    }));
}
