use super::*;

#[test]
#[ignore = "wires are stored as normalized segments, so an extension computed before a cut re-adds the whole wire over it"]
fn a_wire_cut_stays_cut_when_someone_extends_it_at_once() {
    let base = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddWire {
            wire: logic_wire((0, 0), (8, 0)),
        }],
    );
    let cut = logic(
        &base,
        &[LogicGridOperation::RemoveWire {
            wire: logic_wire((3, 0), (5, 0)),
        }],
    );
    let extend = logic(
        &base,
        &[LogicGridOperation::AddWire {
            wire: logic_wire((8, 0), (12, 0)),
        }],
    );

    for sequenced in [
        edited(&base, [cut.clone(), extend.clone()]),
        edited(&base, [extend, cut]),
    ] {
        assert_eq!(
            sequenced.root().grid().wires(),
            [logic_wire((0, 0), (2, 0)), logic_wire((6, 0), (12, 0))]
        );
    }
}
