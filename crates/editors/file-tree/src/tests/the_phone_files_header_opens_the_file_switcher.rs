use block_editor_beui::BarAction;

use super::*;

#[test]
fn the_phone_files_header_opens_the_file_switcher() {
    let fixture = phone();
    assert!(
        !fixture.test.shown("file-tree.files"),
        "with no file open there is nothing to switch to"
    );

    let mut fixture = phone_with_files(2);
    assert_eq!(fixture.test.label("file-tree.files"), "2");
    fixture.test.take_bar_actions();

    fixture.test.click("file-tree.files");
    fixture.settle();

    assert_eq!(
        fixture.test.take_bar_actions(),
        vec![BarAction::Switch],
        "the open files count asks the workspace for its switcher"
    );
}
