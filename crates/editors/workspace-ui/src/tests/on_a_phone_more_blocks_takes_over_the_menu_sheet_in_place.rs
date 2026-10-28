use std::time::Duration;

use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn on_a_phone_more_blocks_takes_over_the_menu_sheet_in_place() {
    let (mut fixture, _) = editor_sized(Some(Vec2::new(390.0, 844.0)));
    fixture.test.catalog(catalog());
    let mut fixture = picking_with(&fixture, filter(), BlockLocation::Root, true);
    fixture.test.settle();
    let menu = fixture.test.rect_of("sheet.handle");

    fixture.test.click("picker.more");
    fixture.test.advance(Duration::from_millis(16));
    let more = fixture.test.rect_of("sheet.handle");
    assert!(
        (more.top() - menu.top()).abs() < 0.5,
        "the More sheet takes over where the menu's sheet stood, with no slide down and up: {more:?} after {menu:?}"
    );
    assert!(
        !fixture.test.shown("picker.more") && fixture.test.shown("picker.search"),
        "showing its own content at once"
    );
    fixture.test.advance(Duration::from_millis(48));
    let moving = fixture.test.rect_of("sheet.handle");
    fixture.test.record();

    fixture.test.settle();
    let settled = fixture.test.rect_of("sheet.handle");
    assert!(
        (moving.top() - more.top()) * (settled.top() - moving.top()) >= 0.0
            && (settled.top() - more.top()).abs() > (moving.top() - more.top()).abs(),
        "then eases to its own height, part way at {moving:?} from {more:?} to {settled:?}"
    );
    fixture.test.record();
    fixture
        .test
        .snapshot("on_a_phone_more_blocks_takes_over_the_menu_sheet_in_place");
}
