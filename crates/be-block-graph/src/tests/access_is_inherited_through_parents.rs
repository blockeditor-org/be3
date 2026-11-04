use super::*;

#[test]
fn access_is_inherited_through_parents() {
    let reader = id(0xbeef);
    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
        (4, BlockParent::Root),
    ]);

    assert_eq!(graph.access(id(3), reader), Access::None);
    graph.grant(id(1), reader, Access::View).unwrap();
    assert_eq!(graph.access(id(3), reader), Access::View);
    assert_eq!(graph.access(id(4), reader), Access::None);

    graph.grant(id(2), reader, Access::Edit).unwrap();
    assert_eq!(graph.access(id(3), reader), Access::Edit);
    assert!(graph.access(id(3), reader).can_view());

    graph.grant(id(2), reader, Access::None).unwrap();
    assert_eq!(graph.access(id(3), reader), Access::View);
    graph.grant(id(1), reader, Access::None).unwrap();
    assert_eq!(graph.access(id(3), reader), Access::None);
}
