use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

#[test]
fn on_a_phone_the_address_bar_fits_across_the_page() {
    let mut tab = Harness::new();
    tab.editor.set_size(PHONE);
    tab.editor.settle();
    tab.editor
        .snapshot("on_a_phone_the_address_bar_fits_across_the_page");
    let go = tab.editor.rect_of("browser.go");
    assert!(
        go.right() <= PHONE.x + 0.5,
        "the whole address bar fits across a phone, {go:?}"
    );

    tab.editor.set_size(LANDSCAPE_PHONE);
    tab.editor.settle();
    assert!(tab.editor.shown("browser.go"));
    tab.editor
        .snapshot("on_a_phone_on_its_side_the_address_bar_fits_across_the_page");
}
