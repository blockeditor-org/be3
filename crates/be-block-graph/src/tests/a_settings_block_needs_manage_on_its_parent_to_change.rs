use super::*;

#[test]
fn a_settings_block_needs_manage_on_its_parent_to_change() {
    let owner = ACCOUNTS[0];
    let editor = ACCOUNTS[1];
    let manager = ACCOUNTS[2];
    let stranger = ACCOUNTS[3];
    let workspace = id(1);
    let folder = id(2);
    let workspace_settings = id(3);
    let folder_settings = id(4);
    let kind = id(5);
    let mut graph = BlockGraph::new();
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
            workspace_settings,
            BlockNode::new(SETTINGS_TYPE, owner, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(
            folder_settings,
            BlockNode::new(SETTINGS_TYPE, editor, BlockParent::Block(folder)),
        )
        .unwrap();
    graph
        .insert(
            kind,
            BlockNode::new(TYPE, owner, BlockParent::Block(workspace_settings)),
        )
        .unwrap();
    graph.grant(workspace, editor, Access::Edit).unwrap();
    graph.grant(folder, manager, Access::Manage).unwrap();
    graph
        .grant(workspace_settings, editor, Access::Edit)
        .unwrap();
    graph
        .grant(workspace_settings, stranger, Access::Manage)
        .unwrap();

    assert_eq!(graph.inherited(workspace_settings, owner), Access::Manage);
    assert_eq!(graph.inherited(kind, owner), Access::Manage);
    assert_eq!(graph.inherited(folder, editor), Access::Edit);
    assert_eq!(graph.inherited(workspace_settings, editor), Access::View);
    assert_eq!(graph.inherited(folder_settings, editor), Access::View);
    assert_eq!(graph.inherited(kind, editor), Access::View);
    assert_eq!(graph.inherited(folder_settings, manager), Access::Manage);
    assert_eq!(graph.inherited(workspace_settings, manager), Access::None);
    assert_eq!(
        graph.effective_access(workspace_settings, stranger),
        Access::KnowExists
    );
    assert_eq!(graph.inherited(kind, stranger), Access::None);
    assert!(!graph.viewers(kind).contains(&stranger));
    assert!(graph.viewers(kind).contains(&editor));
    assert_eq!(graph.visible(editor).get(&kind), Some(&Access::View));
    assert!(graph.shared(stranger).is_empty());
}
