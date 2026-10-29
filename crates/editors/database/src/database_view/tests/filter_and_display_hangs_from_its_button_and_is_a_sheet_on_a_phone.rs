use block_editor_beui::beui::Vec2;

use super::*;

#[test]
fn filter_and_display_hangs_from_its_button_and_is_a_sheet_on_a_phone() {
    let mut fixture = text_editor();
    let harness = &mut fixture.harness;
    harness.set_size(Vec2::new(560.0, 640.0));

    harness.click("database-view.filter-and-display");
    harness.run();
    assert!(harness.shown("database-view.sheet.close"));
    let button = harness.rect_of("database-view.filter-and-display");
    let close = harness.rect_of("database-view.sheet.close");
    assert!(
        close.top() > button.bottom(),
        "on a narrow frame the card hangs below its button: {close:?} under {button:?}"
    );
    harness.snapshot("filter_and_display_hangs_from_its_button");

    harness.click("database-view.sheet.close");
    harness.run();
    harness.set_size(Vec2::new(360.0, 640.0));
    harness.click("database-view.filter-and-display");
    for _ in 0..30 {
        harness.advance(std::time::Duration::from_millis(16));
    }
    let close = harness.rect_of("database-view.sheet.close");
    assert!(
        close.left() >= 0.0 && close.right() <= 360.0,
        "on a phone it is a sheet across the screen: {close:?}"
    );
    harness.snapshot("filter_and_display_is_a_sheet_on_a_phone");
}
