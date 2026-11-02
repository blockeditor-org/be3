use super::*;

#[test]
fn text_types_each_utf16_unit_and_presses_enter_and_tab() {
    let unicode = |unit, up| Synth::Key {
        vk: 0,
        scan: unit,
        flags: KEY_UNICODE | up,
    };
    assert_eq!(text("é"), [unicode(0xE9, 0), unicode(0xE9, KEY_UP)]);
    assert_eq!(
        text("😀"),
        [
            unicode(0xD83D, 0),
            unicode(0xD83D, KEY_UP),
            unicode(0xDE00, 0),
            unicode(0xDE00, KEY_UP)
        ]
    );
    assert_eq!(
        text("\n\t"),
        [
            key(28, true).unwrap(),
            key(28, false).unwrap(),
            key(15, true).unwrap(),
            key(15, false).unwrap()
        ]
    );
}
