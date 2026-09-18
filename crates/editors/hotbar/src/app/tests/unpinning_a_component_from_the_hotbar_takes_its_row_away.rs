use super::*;

#[test]
fn unpinning_a_component_from_the_hotbar_takes_its_row_away() {
    let adder = Uuid::new_v4();
    let slots = vec![
        HotbarSlot::Component {
            name: "Adder".to_owned(),
            compiled: BlockRef::Direct(adder),
        },
        HotbarSlot::Folder {
            name: "Memory".to_owned(),
            slots: vec![HotbarSlot::Component {
                name: "Latch".to_owned(),
                compiled: BlockRef::Direct(adder),
            }],
        },
    ];
    let (mut editor, block) = editor(slots);

    assert!(editor.shown("hotbar.slot.0.unpin"));
    editor.click("hotbar.slot.0.unpin");
    editor.run();

    assert_eq!(
        block.read().unwrap().slots(),
        [HotbarSlot::Folder {
            name: "Memory".to_owned(),
            slots: Vec::new(),
        }]
    );
    editor.snapshot("unpinning_a_component_from_the_hotbar_takes_its_row_away");
}
