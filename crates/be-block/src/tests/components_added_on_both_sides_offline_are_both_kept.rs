use super::*;

#[test]
#[ignore = "each side takes the grid's next component id, and grid() keeps only the first component with a given id"]
fn components_added_on_both_sides_offline_are_both_kept() {
    let base = LogicGridContent::default();
    let id = base.root().grid().next_component_id();
    let ours = logic_run(
        &base,
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 0, 0),
        }],
    );
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 8, 0),
        }],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 0);
    let grid = merged.root().grid();
    assert_eq!(grid.components().count(), 2);
    let ids: std::collections::BTreeSet<ComponentId> =
        grid.components().map(|component| component.id).collect();
    assert_eq!(ids.len(), 2);
}
