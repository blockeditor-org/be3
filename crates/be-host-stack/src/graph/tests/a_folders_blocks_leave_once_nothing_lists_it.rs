use super::*;

#[test]
fn a_folders_blocks_leave_once_nothing_lists_it() {
    let folder = Uuid::from_u128(1);
    let (inside, open) = (Uuid::from_u128(2), Uuid::from_u128(3));
    let under = |id| Node {
        parent: Some(BlockParent::Block(folder)),
        ..node(id)
    };
    let mut graph = Graph::default();
    graph.want(Owner::Editor(1), vec![Query::Children(folder)]);
    assert!(!graph.ready(Query::Children(folder)));
    assert_eq!(
        graph.take_wanted(),
        [
            Listing::Children(BlockParent::Block(folder)),
            Listing::Block(folder)
        ],
        "and the folder's own summary, whose tree is followed"
    );
    graph.listed(Listing::Block(folder), vec![node(folder)], 1);
    graph.listed(
        Listing::Children(BlockParent::Block(folder)),
        vec![under(inside), under(open)],
        1,
    );
    assert!(graph.ready(Query::Children(folder)));
    assert_eq!(graph.query(Query::Children(folder)).len(), 2);

    graph.want(Owner::Open, vec![Query::Block(open)]);
    assert!(
        graph.ready(Query::Block(open)),
        "a block another listing holds is ready at once"
    );
    assert_eq!(
        graph.take_wanted(),
        [Listing::Block(open)],
        "but is still asked for, so the server tells of it on its own"
    );
    graph.listed(Listing::Block(open), vec![under(open)], 1);
    graph.want(Owner::Editor(1), Vec::new());
    assert!(graph.get(inside).is_none(), "nothing lists it any more");
    assert!(graph.get(open).is_some(), "it is still held on its own");

    let later = Uuid::from_u128(4);
    graph.put(changed_at(under(later), 2), 2);
    assert!(
        graph.get(later).is_none(),
        "news of a block in a folder nobody lists is not kept"
    );
    graph.want(Owner::Editor(1), vec![Query::Children(folder)]);
    graph.listed(
        Listing::Children(BlockParent::Block(folder)),
        vec![under(inside), under(open)],
        3,
    );
    graph.put(changed_at(under(later), 4), 4);
    assert!(graph.get(later).is_some(), "a new block in a listed folder");
    graph.put(
        Node {
            parent: Some(BlockParent::Root),
            ..changed_at(node(later), 5)
        },
        5,
    );
    assert!(
        graph.get(later).is_none(),
        "moving out of the folder takes it out of the listing"
    );
}
