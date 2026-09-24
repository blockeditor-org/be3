use crate::logic_grid::{LogicGridContent, LogicGridOperation};
use crate::{ChildChange, Root};
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, ComponentPort, ComponentSide,
    Point, Scale, Size, Wire,
};
use uuid::Uuid;

fn wire(start: (i64, i64), end: (i64, i64)) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::ONE,
    )
    .unwrap()
}

fn run(content: &mut LogicGridContent, operations: &[LogicGridOperation]) {
    let edit = content.root().edit_for_all(operations);
    content.apply(&edit);
}

#[test]
fn a_logic_grid_edit_keeps_its_wires_normalized_and_follows_its_children() {
    let compiled = Uuid::new_v4();
    let mut content = LogicGridContent::default();
    run(
        &mut content,
        &[
            LogicGridOperation::AddWire {
                wire: wire((0, 0), (4, 0)),
            },
            LogicGridOperation::AddWire {
                wire: wire((4, 0), (8, 0)),
            },
            LogicGridOperation::AddComponent {
                component: Component {
                    id: ComponentId(3),
                    position: Point::new(0, 8),
                    orientation: ComponentOrientation::Up,
                    kind: ComponentKind::subcomponent(
                        compiled,
                        Size::new(2, 2),
                        vec![ComponentPort::input(
                            0,
                            Scale::ONE,
                            ComponentSide::Left,
                            0,
                            1,
                        )],
                    )
                    .unwrap(),
                },
            },
        ],
    );
    assert_eq!(content.root().grid().wires(), [wire((0, 0), (8, 0))]);
    assert_eq!(content.root().wires.len(), 1);
    assert_eq!(content.root().references(), [compiled]);

    run(
        &mut content,
        &[LogicGridOperation::RemoveWire {
            wire: wire((3, 0), (5, 0)),
        }],
    );
    assert_eq!(
        content.root().grid().wires(),
        [wire((0, 0), (2, 0)), wire((6, 0), (8, 0))]
    );

    let replaced = Uuid::new_v4();
    let edit = content
        .root()
        .child_edit(ChildChange::Replace {
            old: compiled,
            new: replaced,
        })
        .unwrap();
    content.apply(&edit);
    assert_eq!(content.root().references(), [replaced]);

    let edit = content
        .root()
        .child_edit(ChildChange::Delete(replaced))
        .unwrap();
    content.apply(&edit);
    assert!(content.root().grid().component(ComponentId(3)).is_none());
}
