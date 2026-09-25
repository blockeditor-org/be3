use super::*;

#[test]
fn wires_drawn_on_each_side_that_meet_join_into_one() {
    let base = LogicGridContent::default();
    let ours = logic_run(
        &base,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (4, 0)),
        }],
    );
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((4, 0), (8, 0)),
        }],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().grid().wires(), [logic_wire((0, 0), (8, 0))]);
    let cut = logic_run(
        &merged,
        &[LogicGridOperation::RemoveWire {
            wire: logic_wire((3, 0), (5, 0)),
        }],
    );
    assert_eq!(
        cut.root().grid().wires(),
        [logic_wire((0, 0), (2, 0)), logic_wire((6, 0), (8, 0))]
    );
}
