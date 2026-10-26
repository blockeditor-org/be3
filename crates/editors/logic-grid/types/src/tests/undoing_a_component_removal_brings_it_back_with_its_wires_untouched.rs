use super::*;

#[test]
fn undoing_a_component_removal_brings_it_back_with_its_wires_untouched() {
    let id = ComponentId(0);
    let mut content = logic_run(
        &LogicGridContent::default(),
        &[
            LogicGridOperation::AddComponent {
                component: not_gate(id, 0, 0),
            },
            LogicGridOperation::AddWire {
                wire: logic_wire((0, 4), (8, 4)),
            },
        ],
    );
    let before = content.root().grid();

    let remove = logic(&content, &[LogicGridOperation::RemoveComponent { id }]);
    let step = undone(&mut content, remove);
    assert!(content.root().grid().component(id).is_none());

    reverted(&mut content, &step);
    assert_eq!(content.root().grid(), before);
}
