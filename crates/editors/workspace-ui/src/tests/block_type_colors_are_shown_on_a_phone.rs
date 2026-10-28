use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn block_type_colors_are_shown_on_a_phone() {
    let (mut fixture, _) = editor_sized(Some(Vec2::new(390.0, 844.0)));
    let mut types = catalog();
    types
        .types
        .iter_mut()
        .find(|entry| entry.block_type == TEMPLATE_TYPE.into_bytes())
        .unwrap()
        .color = Some([203, 91, 125]);
    fixture.test.catalog(types);
    let mut fixture = picking_with(&fixture, filter(), BlockLocation::Root, true);
    fixture.test.settle();
    fixture
        .test
        .snapshot("block_type_colors_in_the_phone_add_menu");
    fixture.test.click("picker.more");
    fixture.test.settle();
    fixture
        .test
        .snapshot("block_type_colors_in_the_phone_full_list");
}
