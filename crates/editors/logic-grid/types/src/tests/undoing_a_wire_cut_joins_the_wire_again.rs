use super::*;

#[test]
fn undoing_a_wire_cut_joins_the_wire_again() {
    let mut content = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (8, 0)),
        }],
    );

    let cut = logic(
        &content,
        &[LogicGridOperation::RemoveWire {
            wire: logic_wire((3, 0), (5, 0)),
        }],
    );
    let step = undone(&mut content, cut);
    assert_eq!(content.root().grid().wires().len(), 2);

    reverted(&mut content, &step);
    assert_eq!(content.root().grid().wires(), [logic_wire((0, 0), (8, 0))]);
}
