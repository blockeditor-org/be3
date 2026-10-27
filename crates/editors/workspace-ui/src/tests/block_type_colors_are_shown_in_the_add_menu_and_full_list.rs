use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn block_type_colors_are_shown_in_the_add_menu_and_full_list() {
    let (mut fixture, _) = editor();
    let mut types = catalog();
    types
        .types
        .iter_mut()
        .find(|entry| entry.block_type == TEMPLATE_TYPE.into_bytes())
        .unwrap()
        .color = Some([203, 91, 125]);
    fixture.test.catalog(types);
    let mut fixture = picking(&fixture, filter(), BlockLocation::Root);
    fixture.test.settle();
    fixture.test.snapshot("block_type_colors_in_the_add_menu");
    fixture.test.click("picker.more");
    fixture.test.settle();
    fixture.test.snapshot("block_type_colors_in_the_full_list");
}
