use super::*;

const NATURAL: &str = "input.pointer.natural-scroll";

#[test]
fn every_pointer_reads_as_mixed_until_it_is_set() {
    let mut scrolls_naturally = touchpad();
    scrolls_naturally.defaults.natural_scroll = Some(true);
    let mut editor = editor(vec![mouse(), scrolls_naturally]);
    editor.snapshot("every_pointer_reads_as_mixed_until_it_is_set");
    let mouse = InputDevice {
        name: "Mouse".to_owned(),
        vendor: 0x046d,
        product: 0xc52b,
    };

    editor.click(NATURAL);
    let root = content(&editor).root();
    assert_eq!(
        root.every_pointer.natural_scroll,
        Some(true),
        "a mixed switch turns on"
    );

    choose_pointer(&mut editor, 1);
    editor.click(NATURAL);
    let root = content(&editor).root();
    assert_eq!(root.pointer(&mouse).natural_scroll, Some(false));
    assert_eq!(
        root.pointer(&touchpad_identity()).natural_scroll,
        Some(true)
    );

    choose_pointer(&mut editor, 0);
    reset(&mut editor, NATURAL);
    let root = content(&editor).root();
    assert_eq!(root.every_pointer.natural_scroll, None);
    assert!(
        root.pointers.is_empty(),
        "resetting every pointer resets each override too"
    );
}
