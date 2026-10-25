use super::*;

#[test]
fn a_kind_an_editor_changes_hangs_from_its_owner() {
    let owner = ACCOUNTS[0];
    let editor = ACCOUNTS[1];
    let guest = ACCOUNTS[2];
    let workspace = id(1);
    let note = id(2);
    let settings = id(3);
    let home = id(4);
    let tags = id(5);
    let tag = id(6);
    let mut graph = BlockGraph::new();
    graph
        .insert(workspace, BlockNode::new(TYPE, owner, BlockParent::Root))
        .unwrap();
    graph
        .insert(note, BlockNode::new(TYPE, owner, BlockParent::Block(workspace)))
        .unwrap();
    graph
        .insert(
            settings,
            BlockNode::new(SETTINGS_TYPE, owner, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(home, BlockNode::new(TYPE, owner, BlockParent::Block(settings)))
        .unwrap();
    graph
        .insert(
            tags,
            BlockNode::new(TAGS_TYPE, editor, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(tag, BlockNode::new(TYPE, editor, BlockParent::Block(tags)))
        .unwrap();
    graph.grant(workspace, editor, Access::Edit).unwrap();
    graph.grant(note, guest, Access::View).unwrap();

    assert_eq!(graph.inherited(tags, editor), Access::Edit);
    assert!(graph.inherited(tag, editor).can_edit());
    assert_eq!(graph.inherited(home, editor), Access::View);
    assert_eq!(graph.inherited(tags, owner), Access::Manage);
    assert_eq!(graph.settings_owner(tags), Some(workspace));
    assert_eq!(graph.kinds_of(workspace), vec![home, tags]);
    assert!(!graph.is_inside(workspace, tag));

    assert_eq!(graph.inherited(tag, guest), Access::None);
    graph.grant(tags, ANYONE_INSIDE, Access::View).unwrap();
    assert_eq!(graph.inherited(tags, guest), Access::View);
    assert_eq!(graph.inherited(tag, guest), Access::View);
    assert_eq!(graph.visible(guest).get(&tag), Some(&Access::View));
    assert!(graph.viewers(tag).contains(&guest));
    assert!(!graph.inside_viewers(workspace).contains(&ANYONE_INSIDE));
}
