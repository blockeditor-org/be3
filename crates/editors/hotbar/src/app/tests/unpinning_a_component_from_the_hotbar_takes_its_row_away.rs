use super::*;
use block_editor_plugin::be_block::BlockRef;
use block_editor_plugin::be_block::hotbar::SlotKind;

#[test]
fn unpinning_a_component_from_the_hotbar_takes_its_row_away() {
    let adder = BlockRef::Direct(Uuid::new_v4());
    let mut editor = editor(vec![
        HotbarSlot::component("Adder", adder),
        HotbarSlot::folder("Memory", [HotbarSlot::component("Latch", adder)]),
    ]);

    assert!(editor.shown("hotbar.slot.0.unpin"));
    editor.click("hotbar.slot.0.unpin");
    editor.run();

    let slots = editor.content::<HotbarContent>(None).root().slots;
    assert_eq!(slots.len(), 1);
    assert_eq!(
        slots[0].kind,
        SlotKind::Folder {
            name: "Memory".to_owned()
        }
    );
    assert!(slots[0].slots.is_empty());
    editor.snapshot("unpinning_a_component_from_the_hotbar_takes_its_row_away");
}
