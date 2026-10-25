use super::*;

#[test]
fn a_kind_open_inside_is_read_by_anyone_who_views_a_block_inside() {
    let owner = ACCOUNTS[0];
    let guest = ACCOUNTS[1];
    let stranger = ACCOUNTS[2];
    let workspace = id(1);
    let folder = id(2);
    let note = id(3);
    let settings = id(4);
    let open = id(5);
    let closed = id(6);
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
            note,
            BlockNode::new(TYPE, owner, BlockParent::Block(folder)),
        )
        .unwrap();
    graph
        .insert(
            settings,
            BlockNode::new(SETTINGS_TYPE, owner, BlockParent::Block(workspace)),
        )
        .unwrap();
    for kind in [open, closed] {
        graph
            .insert(
                kind,
                BlockNode::new(TYPE, owner, BlockParent::Block(settings)),
            )
            .unwrap();
    }
    graph.grant(open, ANYONE_INSIDE, Access::View).unwrap();
    graph.grant(note, guest, Access::View).unwrap();
    graph.grant(settings, stranger, Access::View).unwrap();

    assert_eq!(graph.settings_of(workspace), vec![settings]);
    assert_eq!(graph.settings_owner(open), Some(workspace));
    assert_eq!(graph.inherited(open, guest), Access::View);
    assert_eq!(graph.inherited(closed, guest), Access::None);
    assert_eq!(graph.inherited(settings, guest), Access::None);
    assert_eq!(graph.inherited(open, stranger), Access::None);
    assert_eq!(graph.visible(guest).get(&open), Some(&Access::View));
    assert!(!graph.visible(guest).contains_key(&closed));
    assert!(graph.viewers(open).contains(&guest));
    assert!(!graph.viewers(open).contains(&ANYONE_INSIDE));
    assert!(!graph.viewers(closed).contains(&guest));
    assert!(graph.shared(guest).contains(&note));
    assert!(!graph.shared(guest).contains(&open));

    graph.grant(open, ANYONE_INSIDE, Access::None).unwrap();
    assert_eq!(graph.inherited(open, guest), Access::None);
}
