use super::*;

#[test]
fn a_lower_grant_never_reduces_inherited_access() {
    let reader = id(0xbeef);
    let levels = [
        Access::KnowExists,
        Access::View,
        Access::Edit,
        Access::Manage,
    ];
    for above in levels {
        for below in levels {
            let mut graph = graph(&[
                (1, BlockParent::Root),
                (2, BlockParent::Block(id(1))),
                (3, BlockParent::Block(id(2))),
                (4, BlockParent::Block(id(3))),
            ]);
            graph.grant(id(1), reader, above).unwrap();
            graph.grant(id(3), reader, below).unwrap();
            let inherited = match above.can_view() {
                true => above,
                false => Access::None,
            };
            assert_eq!(graph.inherited(id(2), reader), inherited);
            for block in [id(3), id(4)] {
                let expected = match inherited.can_view() {
                    true => above.max(below),
                    false if block == id(3) => below,
                    false if below.can_view() => below,
                    false => Access::None,
                };
                assert_eq!(
                    graph.inherited(block, reader),
                    expected,
                    "{above:?} above and {below:?} below, at {block}"
                );
            }
        }
    }

    let mut graph = graph(&[
        (1, BlockParent::Root),
        (2, BlockParent::Block(id(1))),
        (3, BlockParent::Block(id(2))),
    ]);
    graph.grant(id(1), reader, Access::Manage).unwrap();
    for block in graph.subtree(id(1)) {
        assert_eq!(graph.inherited(block, reader), Access::Manage);
        assert!(graph.inherited(block, reader).can_manage());
    }
}
