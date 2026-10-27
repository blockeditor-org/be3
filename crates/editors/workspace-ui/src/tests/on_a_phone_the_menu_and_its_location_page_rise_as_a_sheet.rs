use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn on_a_phone_the_menu_and_its_location_page_rise_as_a_sheet() {
    let (mut fixture, _) = editor_sized(Some(Vec2::new(390.0, 844.0)));
    fixture.test.catalog(catalog());
    let places = places(&mut fixture);
    let mut placing = filter();
    placing.place = Some(BlockLocation::Block(places.work.into_bytes()));
    let mut fixture = picking_with(&fixture, placing, BlockLocation::Detached, true);
    fixture.test.settle();
    fixture.test.record();

    fixture.test.click("picker.location");
    fixture.test.settle();
    assert!(fixture.test.shown(&format!("picker.place.{}", places.q4)));
    fixture.test.record();
    fixture
        .test
        .snapshot("on_a_phone_the_menu_and_its_location_page_rise_as_a_sheet");
}
