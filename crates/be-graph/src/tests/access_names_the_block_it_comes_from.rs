use super::*;

#[test]
fn access_names_the_block_it_comes_from() {
    let reader = id(0xbeef);
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
        (4, BlockParent::Block(id(3))),
    ]);
    graph.grant(id(1), reader, Access::Edit).unwrap();
    graph.grant(id(3), reader, Access::View).unwrap();
    assert_eq!(
        graph.inherited_from(id(4), reader),
        (Access::Edit, Some(id(1))),
        "a lower grant below does not take the source"
    );
    graph.grant(id(3), reader, Access::Manage).unwrap();
    assert_eq!(
        graph.inherited_from(id(4), reader),
        (Access::Manage, Some(id(3)))
    );
    assert_eq!(
        graph.inherited_from(id(3), reader),
        (Access::Manage, Some(id(3)))
    );
    assert_eq!(
        graph.inherited_from(id(1), AUTHOR),
        (Access::Manage, Some(id(1)))
    );
    assert_eq!(graph.inherited_from(id(4), id(0xf00)), (Access::None, None));

    assert_eq!(
        graph.set_inside(id(1)),
        1,
        "only the blocks below are counted"
    );
    assert_eq!(graph.set_inside(id(3)), 0);
    graph
        .insert(
            id(5),
            BlockNode::new(SETTINGS_TYPE, AUTHOR, BlockParent::Block(id(2))),
        )
        .unwrap();
    graph.grant(id(5), reader, Access::Edit).unwrap();
    assert_eq!(
        graph.set_inside(id(1)),
        1,
        "a settings block's own grants set nothing"
    );
    assert_eq!(graph.settings_of(id(2)), vec![id(5)]);
    assert_eq!(graph.settings_of(id(3)), Vec::<Uuid>::new());
}
