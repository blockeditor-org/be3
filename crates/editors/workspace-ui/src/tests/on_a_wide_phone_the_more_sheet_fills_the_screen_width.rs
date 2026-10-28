use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn on_a_wide_phone_the_more_sheet_fills_the_screen_width() {
    let width = 600.0;
    let (mut fixture, _) = editor_sized(Some(Vec2::new(width, 900.0)));
    fixture.test.catalog(catalog());
    let mut fixture = picking_with(&fixture, filter(), BlockLocation::Root, true);
    fixture.test.settle();
    fixture.test.click("picker.more");
    fixture.test.settle();
    let search = fixture.test.rect_of("picker.search");
    assert!(
        search.left() < 40.0 && search.right() > width - 40.0,
        "the More card falls back to a sheet whose content fills the screen's width rather than keeping the card's: {search:?}"
    );
    fixture
        .test
        .snapshot("on_a_wide_phone_the_more_sheet_fills_the_screen_width");
}
