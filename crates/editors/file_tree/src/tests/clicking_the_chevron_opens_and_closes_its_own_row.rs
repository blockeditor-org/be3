use block_editor_plugin::beui::icons::{ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};

use super::*;

#[test]
fn clicking_the_chevron_opens_and_closes_its_own_row() {
    let mut fixture = editor();
    let row = fixture.test.rect_of("file-tree.row.orphans");
    assert_eq!(
        fixture.test.label("file-tree.chevron.orphans"),
        format!("{ICON_KEYBOARD_ARROW_RIGHT} Expand"),
        "a closed row shows a chevron that says what it does"
    );

    fixture.test.click("file-tree.chevron.orphans");
    fixture.settle();

    assert_eq!(
        fixture.test.label("file-tree.chevron.orphans"),
        format!("{ICON_KEYBOARD_ARROW_DOWN} Collapse"),
        "the chevron must expand the row it belongs to"
    );
    assert!(
        fixture.opened().is_empty(),
        "expanding a row must not open anything"
    );
    assert_eq!(
        fixture.test.rect_of("file-tree.row.orphans").height(),
        row.height(),
        "expanding must leave the row it was asked of the height it was"
    );

    fixture.test.click("file-tree.chevron.orphans");
    fixture.settle();

    assert_eq!(
        fixture.test.label("file-tree.chevron.orphans"),
        format!("{ICON_KEYBOARD_ARROW_RIGHT} Expand"),
        "the chevron must collapse the row again"
    );
}
