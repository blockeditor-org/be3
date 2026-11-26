use super::*;

#[test]
fn a_tree_is_followed_while_the_mirror_holds_what_lies_in_it() {
    let (entry, folder, other) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let (inside, made, elsewhere) = (Uuid::from_u128(4), Uuid::from_u128(5), Uuid::from_u128(6));
    let in_tree = |id, parent| Node {
        parent: Some(BlockParent::Block(parent)),
        entry: Some(entry),
        ..node(id)
    };
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::Children(folder)]);
    graph.take_wanted();
    graph.listed(
        Listing::Children(BlockParent::Block(folder)),
        vec![changed_at(in_tree(inside, folder), 3)],
        5,
    );
    assert_eq!(
        graph.take_follows(),
        (vec![(entry, 5)], Vec::new()),
        "the tree of what is listed is followed from the sequence it was listed at"
    );

    // Another device fills the folder and another one in the same tree; the read lands.
    let departed = graph.followed(
        entry,
        vec![
            changed_at(in_tree(made, folder), 7),
            changed_at(in_tree(elsewhere, other), 8),
        ],
        &[],
        8,
    );
    assert!(departed.is_empty());
    assert!(graph.get(made).is_some(), "a block the folder lists joins");
    assert!(
        graph.get(elsewhere).is_none(),
        "a block of the tree that nothing shown lists is left out"
    );
    graph.advanced(entry, 8);
    assert!(
        !graph.has_follows_to_take(),
        "a tree read to the sequence it advanced to is not read again"
    );

    // A block that left the tree is read again wherever it went.
    graph.advanced(entry, 9);
    assert_eq!(graph.take_follows(), (vec![(entry, 8)], Vec::new()));
    assert_eq!(graph.followed(entry, Vec::new(), &[inside], 9), [inside]);

    graph.want(Owner::Editor(1), Vec::new());
    assert_eq!(
        graph.take_follows(),
        (Vec::new(), vec![entry]),
        "once nothing held lies in the tree, it is unfollowed"
    );
}
