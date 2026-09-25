use super::*;

#[test]
#[ignore = "a merge keeps an object or map entry one side deleted when the other side edited it, instead of taking the delete and counting a conflict"]
fn a_component_removed_on_one_side_and_turned_on_the_other_stays_removed() {
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

    assert_eq!(merged.root().grid().components().count(), 0);
}
