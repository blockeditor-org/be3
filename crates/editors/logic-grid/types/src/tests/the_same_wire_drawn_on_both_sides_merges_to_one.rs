use super::*;

#[test]
fn the_same_wire_drawn_on_both_sides_merges_to_one() {
    let base = LogicGridContent::default();
    let draw = [LogicGridOperation::AddWire {
        wire: logic_wire((0, 0), (8, 0)),
    }];
    let ours = logic_run(&base, &draw);
    let theirs = logic_run(&base, &draw);

    let (merged, conflicts) = merged(&base, &ours, &theirs);
    let sequenced = edited(&base, [logic(&base, &draw), logic(&base, &draw)]);

    assert_eq!(conflicts, 0);
    assert_eq!(merged.root().grid().wires(), [logic_wire((0, 0), (8, 0))]);
    assert_eq!(
        sequenced.root().grid().wires(),
        [logic_wire((0, 0), (8, 0))]
    );
}
