use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);
const TIC_TAC_TOE: &[u8] = include_bytes!(env!("TIC_TAC_TOE_WASM"));

#[test]
fn on_a_phone_the_replace_panel_is_gathered_beside_the_module() {
    let (mut test, editor) = editor(TIC_TAC_TOE.to_vec());
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
