use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);
const TIC_TAC_TOE: &[u8] = include_bytes!(env!("TIC_TAC_TOE_WASM"));

#[test]
fn on_a_phone_the_replace_panel_is_gathered_beside_the_module() {
    // The view shows the module's size, so the paintings would move with every compiler change.
    let (mut test, editor) = editor(padded(TIC_TAC_TOE, 250_000));
    test.set_size(PHONE);
    test.settle();
    let gathered = test.rect_of("dock.compact");
    assert!(
        test.shown("game-module.name")
            && test.shown("game-module.replace")
            && editor.content_rect().bottom() <= gathered.top() + 1.0,
        "the replace panel is gathered below the module, {gathered:?}"
    );
    test.snapshot("on_a_phone_the_replace_panel_is_gathered_below_the_module");

    test.set_size(LANDSCAPE_PHONE);
    test.settle();
    let gathered = test.rect_of("dock.compact");
    assert!(
        test.shown("game-module.replace") && editor.content_rect().right() <= gathered.left() + 1.0,
        "on a phone on its side the replace panel is gathered right of the module, {gathered:?}"
    );
    test.snapshot("on_a_phone_on_its_side_the_replace_panel_is_gathered_right_of_the_module");
}

/// `module` with a custom section appended, which a module may carry and a game ignores, so it is
/// exactly `size` bytes long.
fn padded(module: &[u8], size: usize) -> Vec<u8> {
    const NAME: &[u8] = b"padding";
    // The section's id, its length as a padded five-byte LEB128, then its name.
    let header = 1 + 5 + 1 + NAME.len();
    assert!(
        module.len() + header <= size,
        "the module is {} bytes, too many to pad to {size}",
        module.len()
    );
    let length = u32::try_from(size - module.len() - 1 - 5).expect("a section's length is a u32");
    let mut padded = module.to_vec();
    padded.push(0);
    padded.extend((0..5).map(|byte| {
        let bits = (length >> (7 * byte)) as u8 & 0x7f;
        if byte < 4 { bits | 0x80 } else { bits }
    }));
    padded.push(NAME.len() as u8);
    padded.extend(NAME);
    padded.resize(size, 0);
    padded
}
