use super::*;

#[test]
fn a_component_removed_on_one_side_and_turned_on_the_other_is_kept() {
    let id = ComponentId(0);
    let base = logic_run(
        &LogicGridContent::default(),
        &[LogicGridOperation::AddComponent {
            component: not_gate(id, 0, 0),
        }],
    );
    let ours = logic_run(&base, &[LogicGridOperation::RemoveComponent { id }]);
    let theirs = logic_run(
        &base,
        &[LogicGridOperation::OrientComponent {
            id,
            orientation: ComponentOrientation::Right,
        }],
    );

    let (merged, conflicts) = merged(&base, &ours, &theirs);

    assert_eq!(conflicts, 1);
    assert_eq!(
        merged
            .root()
            .grid()
            .component(id)
            .map(|component| component.orientation),
        Some(ComponentOrientation::Right)
    );
}
