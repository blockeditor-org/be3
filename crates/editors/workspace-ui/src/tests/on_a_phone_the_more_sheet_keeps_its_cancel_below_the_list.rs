use block_editor_beui::BlockLocation;

use super::*;

#[test]
fn on_a_phone_the_more_sheet_keeps_its_cancel_below_the_list() {
    let size = Vec2::new(390.0, 560.0);
    let (mut fixture, _) = editor_sized(Some(size));
    let mut types = catalog();
    let note = types.templates[0].clone();
    types
        .templates
        .extend((0..30).map(|index| block_editor_beui::TemplateDescriptor {
            template: format!("note-{index}"),
            name: format!("Note {index}"),
            category: block_editor_beui::TemplateCategory::Regular,
            ..note.clone()
        }));
    fixture.test.catalog(types);
    let mut fixture = picking_with(&fixture, filter(), BlockLocation::Root, true);
    fixture.test.click("picker.more");
    fixture.test.settle();
    let cancel = fixture.test.rect_of("picker.cancel");
    assert!(
        cancel.bottom() <= size.y && cancel.top() > size.y / 2.0,
        "Cancel stays in view at the foot of the sheet, below the list that scrolls: {cancel:?}"
    );
    fixture
        .test
        .snapshot("on_a_phone_the_more_sheet_keeps_its_cancel_below_the_list");
}
