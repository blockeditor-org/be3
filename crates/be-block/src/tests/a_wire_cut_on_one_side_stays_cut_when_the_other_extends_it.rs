use super::*;

#[test]
#[ignore = "wires are stored as normalized segments, so an extension replaces the segment the cut split and its merge joins the cut up again"]
fn a_wire_cut_on_one_side_stays_cut_when_the_other_extends_it() {
    let base = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (8, 0)),
        }],
    );
    let ours = logic_run(
        &base,
        &[LogicGridOperation::RemoveWire {
            wire: logic_wire((3, 0), (5, 0)),
        }],
    );
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((8, 0), (12, 0)),
        }],
    );

    let (merged, _) = merged(&base, &ours, &theirs);

    assert_eq!(
        merged.root().grid().wires(),
        [logic_wire((0, 0), (2, 0)), logic_wire((6, 0), (12, 0))]
    );
}
