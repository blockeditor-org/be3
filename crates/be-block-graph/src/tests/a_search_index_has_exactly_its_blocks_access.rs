use super::*;

#[test]
fn a_search_index_has_exactly_its_blocks_access() {
    let owner = ACCOUNTS[0];
    let editor = ACCOUNTS[1];
    let viewer = ACCOUNTS[2];
    let inside = ACCOUNTS[3];
    let workspace = id(1);
    let index = id(2);
    let note = id(3);
    let mut graph = BlockGraph::new();
    graph
        .insert(workspace, BlockNode::new(TYPE, owner, BlockParent::Root))
        .unwrap();
    graph
        .insert(
            index,
            BlockNode::new(SEARCH_INDEX_TYPE, editor, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(
            note,
            BlockNode::new(TYPE, owner, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph.set_references(workspace, &[index, note]).unwrap();
    graph.grant(workspace, editor, Access::Edit).unwrap();
    graph.grant(workspace, viewer, Access::View).unwrap();
    graph.grant(note, inside, Access::Manage).unwrap();
    graph.grant(index, viewer, Access::Manage).unwrap();
    graph.grant(index, inside, Access::View).unwrap();

    assert_eq!(graph.inherited(index, owner), Access::Manage);
    assert_eq!(
        graph.inherited(index, editor),
        Access::Edit,
        "authoring the index raised its author above the block it indexes"
    );
    assert_eq!(
        graph.inherited(index, viewer),
        Access::View,
        "a grant on the index raised it"
    );
    assert_eq!(
        graph.inherited(index, inside),
        Access::None,
        "a block shared alone read the tree's index"
    );
    assert_eq!(graph.visible(inside).get(&index), None);
    assert_eq!(graph.visible(viewer).get(&index), Some(&Access::View));
    assert!(!graph.viewers(index).contains(&inside));
    assert!(graph.viewers(index).contains(&viewer));
    assert!(graph.shared(inside).contains(&note));
    assert!(!graph.shared(inside).contains(&index));
    assert!(!graph.get(index).unwrap().sets_access());
    assert_eq!(
        created_access(
            SEARCH_INDEX_TYPE,
            BlockParent::Block(workspace),
            Access::View,
            false
        ),
        Access::View
    );
}
