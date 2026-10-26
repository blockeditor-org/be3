use super::*;

#[test]
fn a_component_moved_on_both_sides_conflicts_and_keeps_ours() {
    let id = ComponentId(0);
    let base = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 0, 0),
        }],
    );
    let ours = logic_run(
        &base,
        &[LogicGridOperation::MoveComponent {
            id,
            position: Point::new(8, 0),
        }],
    );
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::MoveComponent {
            id,
            position: Point::new(0, 8),
        }],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    let grid = merged.root().grid();
    assert_eq!(grid.components().count(), 1);
    assert_eq!(
        grid.component(id).map(|component| component.position),
        Some(Point::new(8, 0))
    );
}
