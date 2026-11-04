use super::*;

#[test]
fn a_move_without_manage_stays_in_its_workspace_and_raises_nothing() {
    let owner = ACCOUNTS[0];
    let editor = ACCOUNTS[1];
    let workspace = id(1);
    let document = id(2);
    let folder = id(3);
    let own = id(4);
    let elsewhere = id(5);
    let mut graph = BlockGraph::new();
    for (block, author, parent) in [
        (workspace, owner, BlockParent::Root),
        (document, owner, BlockParent::Block(workspace)),
        (folder, owner, BlockParent::Block(workspace)),
        (own, editor, BlockParent::Block(workspace)),
        (elsewhere, editor, BlockParent::Root),
    ] {
        graph
            .insert(block, BlockNode::new(TYPE, author, parent))
            .unwrap();
    }
    graph.grant(workspace, editor, Access::Edit).unwrap();
    let refusal = |graph: &BlockGraph, block, parent| {
        graph.move_refusal(block, parent, editor, graph.inherited(block, editor))
    };

    for (block, parent) in [
        (workspace, BlockParent::Block(elsewhere)),
        (workspace, BlockParent::Detached),
        (document, BlockParent::Block(elsewhere)),
        (document, BlockParent::Root),
    ] {
        assert_eq!(
            refusal(&graph, block, parent),
            Some(MoveRefusal::MovesWorkspace),
            "{block} to {parent:?}"
        );
    }
    assert_eq!(
        refusal(&graph, document, BlockParent::Block(own)),
        Some(MoveRefusal::RaisesOwnAccess)
    );
    assert_eq!(refusal(&graph, document, BlockParent::Block(folder)), None);
    assert_eq!(refusal(&graph, document, BlockParent::Detached), None);
    assert_eq!(refusal(&graph, own, BlockParent::Block(elsewhere)), None);
    assert_eq!(
        graph.move_refusal(
            document,
            BlockParent::Block(elsewhere),
            owner,
            Access::Manage
        ),
        None
    );
}
