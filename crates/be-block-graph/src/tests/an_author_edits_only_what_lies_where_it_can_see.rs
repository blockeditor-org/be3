use super::*;

#[test]
fn an_author_edits_only_what_lies_where_it_can_see() {
    let owner = ACCOUNTS[0];
    let member = ACCOUNTS[1];
    let mut graph = BlockGraph::new();
    let workspace = id(1);
    let folder = id(2);
    let written = id(3);
    let shared = id(4);
    graph
        .insert(workspace, BlockNode::new(TYPE, owner, BlockParent::Root))
        .unwrap();
    graph
        .insert(
            folder,
            BlockNode::new(TYPE, owner, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(
            written,
            BlockNode::new(TYPE, member, BlockParent::Block(folder)),
        )
        .unwrap();
    graph
        .insert(
            shared,
            BlockNode::new(TYPE, owner, BlockParent::Block(folder)),
        )
        .unwrap();

    assert_eq!(graph.inherited(written, member), Access::None);
    assert_eq!(graph.inherited(written, owner), Access::Manage);

    graph.grant(workspace, member, Access::View).unwrap();
    assert_eq!(graph.inherited(written, member), Access::Manage);
    assert_eq!(graph.inherited(shared, member), Access::View);

    graph.grant(workspace, member, Access::None).unwrap();
    graph.grant(shared, member, Access::View).unwrap();
    assert_eq!(graph.inherited(written, member), Access::None);
    assert_eq!(graph.effective_access(folder, member), Access::None);
    assert_eq!(graph.effective_access(workspace, member), Access::None);
    let seen = graph.visible(member);
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        [(shared, Access::View)]
    );

    graph.set_parent(written, BlockParent::Detached).unwrap();
    assert_eq!(graph.inherited(written, member), Access::Manage);
}
