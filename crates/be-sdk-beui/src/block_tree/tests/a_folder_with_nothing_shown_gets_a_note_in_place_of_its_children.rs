use super::*;

/// A folder whose children are all hidden, or failed to list, gets a note in their place; one
/// still waiting gets nothing yet; a folded one is listed without its children.
#[test]
fn a_folder_with_nothing_shown_gets_a_note_in_place_of_its_children() {
    let mut hidden = block(2, 1, &[]);
    hidden.deleted = true;
    let blocks = [
        block(1, 9, &[2]),
        hidden,
        block(3, 9, &[]),
        block(4, 9, &[]),
        block(5, 9, &[2]),
    ];
    let graph = graph(&blocks);
    let mut rows = Vec::new();
    BlockTree {
        children: &mut |block| match block.id.as_u128() {
            3 => Some(Listed::Failed("It failed".to_owned())),
            4 => Some(Listed::Waiting),
            _ => children_of(&graph, block),
        },
        open: &|row| row.block.id.as_u128() != 5,
        shown: &|block| !block.deleted,
        matches: None,
    }
    .walk(
        [1, 3, 4, 5]
            .map(|n| graph[&Uuid::from_u128(n)].clone())
            .to_vec(),
        &[],
        0,
        false,
        &mut rows,
    );
    assert_eq!(
        paths(&rows),
        [vec![1], vec![1, 0], vec![3], vec![3, 0], vec![4], vec![5]]
    );
    assert!(matches!(
        &rows[1],
        TreeEntry::Note { depth: 1, note: TreeNote::Empty, .. }
    ));
    assert!(matches!(
        &rows[3],
        TreeEntry::Note { note: TreeNote::Failed(why), .. } if why == "It failed"
    ));
    let TreeEntry::Block(folded) = &rows[5] else {
        panic!("a block row");
    };
    assert!(!folded.expanded);
    assert_eq!(folded.has_children, Some(false));
}
