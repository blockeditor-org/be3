use block_editor_beui::beui::icons::{ICON_CHEVRON_DOWN, ICON_CHEVRON_RIGHT};

use super::*;

#[test]
fn clicking_the_chevron_opens_and_closes_its_own_row() {
    let mut fixture = editor();
    let row = fixture.test.rect_of("file-tree.orphans.row");
    assert_eq!(
        chevron(&fixture),
        (
            ICON_CHEVRON_RIGHT.as_str().to_owned(),
            Some("Expand".to_owned())
        ),
        "a closed row shows a chevron that says what it does"
    );

    fixture.test.click("file-tree.orphans.chevron");
    fixture.settle();

    assert_eq!(
        chevron(&fixture),
        (
            ICON_CHEVRON_DOWN.as_str().to_owned(),
            Some("Collapse".to_owned())
        ),
        "the chevron must expand the row it belongs to"
    );
    assert!(
        fixture.opened().is_empty(),
        "expanding a row must not open anything"
    );
    assert_eq!(
        fixture.test.rect_of("file-tree.orphans.row").height(),
        row.height(),
        "expanding must leave the row it was asked of the height it was"
    );

    fixture.test.click("file-tree.orphans.chevron");
    fixture.settle();

    assert_eq!(
        chevron(&fixture),
        (
            ICON_CHEVRON_RIGHT.as_str().to_owned(),
            Some("Expand".to_owned())
        ),
        "the chevron must collapse the row again"
    );
}

fn chevron(fixture: &Fixture) -> (String, Option<String>) {
    let id = "file-tree.orphans.chevron";
    (fixture.test.label(id), fixture.test.accessible_label(id))
}
