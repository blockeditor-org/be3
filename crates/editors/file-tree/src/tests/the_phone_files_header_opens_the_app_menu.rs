use block_editor_beui::{ChildContent, HostPanel};

use super::*;

#[test]
fn the_phone_files_header_opens_the_app_menu() {
    let mut fixture = phone();
    fixture.settle();

    fixture.test.click("file-tree.app-menu");
    fixture.settle();

    assert!(
        fixture
            .test
            .placed(|content| *content == ChildContent::Host(HostPanel::AppMenu))
            .is_some(),
        "the header button places the app's menu in a view it anchored"
    );
}
