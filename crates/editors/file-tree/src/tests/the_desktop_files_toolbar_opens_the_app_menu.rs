use block_editor_beui::{ChildContent, HostPanel};

use super::*;

#[test]
fn the_desktop_files_toolbar_opens_the_app_menu() {
    let mut fixture = editor();
    fixture.settle();

    fixture.test.click("file-tree.desktop-app-menu");
    fixture.settle();

    assert!(
        fixture
            .test
            .placed(|content| *content == ChildContent::Host(HostPanel::AppMenu))
            .is_some(),
        "the account button places the app's menu in a view it anchored"
    );
}
