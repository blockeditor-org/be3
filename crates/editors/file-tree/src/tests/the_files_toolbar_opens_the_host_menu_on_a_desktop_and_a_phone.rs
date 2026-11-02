use block_editor_beui::{ChildContent, HostPanel};

use super::*;

#[test]
fn the_files_toolbar_opens_the_host_menu_on_a_desktop_and_a_phone() {
    for mut fixture in [editor(), phone()] {
        fixture.settle();

        fixture.test.click("file-tree.host-menu");
        fixture.settle();

        assert!(
            fixture
                .test
                .placed(|content| *content == ChildContent::Host(HostPanel::Menu))
                .is_some(),
            "the account button places the app's menu in a view it anchored"
        );
    }
}
