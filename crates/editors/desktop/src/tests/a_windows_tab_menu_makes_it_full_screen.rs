use block_editor_beui::beui::PointerButton;

use super::*;

#[test]
fn a_windows_tab_menu_makes_it_full_screen() {
    let mut fixture = Fixture::with_windows(&[1]);
    let tab = {
        let document = fixture.test.document();
        let title = document
            .root()
            .and_then(|root| text_within(&document, root, "todo-1.txt"))
            .expect("the window's tab shows its title");
        document
            .node_rect(title)
            .expect("the title is laid out")
            .center()
    };
    fixture.test.click_with(tab, PointerButton::Secondary);
    fixture.settle();
    assert!(
        fixture.says("Full screen"),
        "the tab's menu offers full screen"
    );
    fixture.test.snapshot("a_windows_tab_menu");

    fixture.click_text("Full screen");
    assert_eq!(
        fixture.test.host_calls(block_ui_test::requested::window),
        [WindowAction::Fullscreen {
            window: HostWindowId(1),
            fullscreen: true
        }],
        "the entry asks the host to make the window fullscreen"
    );
    assert!(fixture.test.shown("dock.fullscreen"));
}
