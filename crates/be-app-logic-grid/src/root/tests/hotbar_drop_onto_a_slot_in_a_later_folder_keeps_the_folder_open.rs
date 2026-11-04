use super::*;

#[test]
fn hotbar_drop_onto_a_slot_in_a_later_folder_keeps_the_folder_open() {
    let mut editor = LogicGridEditor::default();
    editor.select_hotbar_path(vec![3]);
    editor.select_hotbar_path(vec![3, 0]);
    assert_eq!(editor.tool.kind, ToolKind::Not);

    editor.hotbar_drag = Some(vec![0]);
    editor.drop_hotbar_slot(Some(HotbarDropTarget::Slot(vec![3, 0])));

    assert!(matches!(
        &editor.hotbar[2],
        HotbarSlot::Folder { name, slots } if name == "Logic" && matches!(
            slots.as_slice(),
            [HotbarSlot::Builtin(ToolKind::Wire), HotbarSlot::Builtin(ToolKind::Not)]
        )
    ));
    assert_eq!(
        editor.active_hotbar_folder,
        vec![2],
        "the open folder follows its move"
    );
    assert_eq!(
        editor.active_hotbar_slot,
        Some(vec![2, 1]),
        "the chosen tool follows its slot"
    );
    assert_eq!(editor.tool.kind, ToolKind::Not);
}
