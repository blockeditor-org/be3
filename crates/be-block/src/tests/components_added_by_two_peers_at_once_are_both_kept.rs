use super::*;

#[test]
#[ignore = "each peer takes the grid's next component id, and grid() keeps only the first component with a given id"]
fn components_added_by_two_peers_at_once_are_both_kept() {
    let base = LogicGridContent::default();
    let id = base.root().grid().next_component_id();
    let ours = logic(
        &base,
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 0, 0),
        }],
    );
    let theirs = logic(
        &base,
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 8, 0),
        }],
    );

    let sequenced = edited(&base, [ours, theirs]);

    let mut positions: Vec<Point> = sequenced
        .root()
        .grid()
        .components()
        .map(|component| component.position)
        .collect();
    positions.sort();
    assert_eq!(positions, [Point::new(0, 0), Point::new(8, 0)]);
}
