use super::*;

#[test]
fn an_author_keeps_no_more_of_a_settings_kind_than_its_settings() {
    let owner = ACCOUNTS[0];
    let former = ACCOUNTS[1];
    let guest = ACCOUNTS[2];
    let workspace = id(1);
    let settings = id(2);
    let kind = id(3);
    let below = id(4);
    let tags = id(5);
    let tag = id(6);
    let mut graph = BlockGraph::new();
    graph
        .insert(workspace, BlockNode::new(TYPE, owner, BlockParent::Root))
        .unwrap();
    graph
        .insert(
            settings,
            BlockNode::new(SETTINGS_TYPE, former, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(kind, BlockNode::new(TYPE, former, BlockParent::Block(settings)))
        .unwrap();
    graph
        .insert(below, BlockNode::new(TYPE, former, BlockParent::Block(kind)))
        .unwrap();
    graph
        .insert(
            tags,
            BlockNode::new(TAGS_TYPE, former, BlockParent::Block(workspace)),
        )
        .unwrap();
    graph
        .insert(tag, BlockNode::new(TYPE, former, BlockParent::Block(tags)))
        .unwrap();
    graph.grant(workspace, former, Access::Manage).unwrap();
    assert_eq!(graph.inherited(kind, former), Access::Manage);
    assert_eq!(graph.inherited(below, former), Access::Manage);

    graph.grant(workspace, former, Access::View).unwrap();
    graph.grant(kind, guest, Access::Edit).unwrap();
    graph.grant(tag, guest, Access::View).unwrap();
    for block in [settings, kind, below, tags, tag] {
        assert_eq!(graph.inherited(block, former), Access::View, "{block}");
        assert_eq!(graph.visible(former).get(&block), Some(&Access::View));
    }
    for block in [kind, below, tag] {
        assert!(!graph.inherited(block, guest).can_view(), "{block}");
        assert!(!graph.viewers(block).contains(&guest));
    }
    assert!(graph.shared(guest).is_empty());
}
