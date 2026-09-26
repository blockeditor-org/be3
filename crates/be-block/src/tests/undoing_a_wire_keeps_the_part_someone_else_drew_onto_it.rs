use super::*;

#[test]
#[ignore = "an extension replaces the drawn segment with a longer one, so the undo finds nothing of its own to remove"]
fn undoing_a_wire_keeps_the_part_someone_else_drew_onto_it() {
    let mut content = LogicGridContent::default();

    let draw = logic(
        &content,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (4, 0)),
        }],
    );
    let step = undone(&mut content, draw);
    let extend = logic(
        &content,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((4, 0), (8, 0)),
        }],
    );
    content.apply(&extend);
    reverted(&mut content, &step);

    assert_eq!(content.root().grid().wires(), [logic_wire((4, 0), (8, 0))]);
}
