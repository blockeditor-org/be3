use super::*;

#[test]
#[ignore = "a branch splits the stored wire into new segments, so the merge keeps both halves of the wire the other side removed"]
fn a_wire_removed_on_one_side_stays_removed_when_the_other_branches_off_it() {
    let base = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (8, 0)),
        }],
    );
    let ours = logic_run(
        &base,
        &[LogicGridOperation::RemoveWire {
            wire: logic_wire((0, 0), (8, 0)),
        }],
    );
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((4, 0), (4, 6)),
        }],
    );
    assert_eq!(ours.root().grid().wires(), []);
    assert_eq!(theirs.root().grid().wires().len(), 3);

    let (merged, _) = merged(&base, &ours, &theirs);

    assert_eq!(merged.root().grid().wires(), [logic_wire((4, 0), (4, 6))]);
}
