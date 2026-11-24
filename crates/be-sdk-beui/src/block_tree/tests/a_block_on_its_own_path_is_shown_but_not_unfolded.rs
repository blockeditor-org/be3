use super::*;

#[test]
fn a_block_on_its_own_path_is_shown_but_not_unfolded() {
    let blocks = [block(1, 9, &[2]), block(2, 1, &[1])];
    let graph = graph(&blocks);
    let mut rows = Vec::new();
    BlockTree {
        children: &mut |block| children_of(&graph, block),
        open: &|_| true,
        shown: &|_| true,
        matches: None,
    }
    .walk(vec![blocks[0].clone()], &[], 0, false, &mut rows);
    assert_eq!(paths(&rows), [vec![1], vec![1, 2], vec![1, 2, 1]]);
    let TreeEntry::Block(again) = &rows[2] else {
        panic!("a block row");
    };
    assert!(!again.expanded);
    assert_eq!(again.has_children, None);
    assert_eq!(again.container, Some(Uuid::from_u128(2)));
}
