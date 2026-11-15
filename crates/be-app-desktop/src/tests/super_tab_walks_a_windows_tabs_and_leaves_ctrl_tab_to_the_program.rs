use be_sdk_beui::beui::Pos2;
use be_sdk_beui::beui::dock::TabId;
use be_sdk_beui::beui::dock_host::styled::DockApi;

use super::*;

fn tab_rect(fixture: &Fixture, title: &str) -> Rect {
    let document = fixture.test.document();
    let node = document
        .root()
        .and_then(|root| text_within(&document, root, title))
        .unwrap_or_else(|| panic!("{title} has a tab"));
    document.node_rect(node).expect("the tab is laid out")
}

fn shown_tab(fixture: &Fixture) -> Option<TabId> {
    let document = fixture.test.document();
    let dock = document
        .find_test_id("desktop.dock")
        .expect("the desktop has a dock");
    let state = document.api::<DockApi>(dock).state();
    let leaf = state.focused_leaf()?;
    state.active_tab(leaf)
}

#[test]
fn super_tab_walks_a_windows_tabs_and_leaves_ctrl_tab_to_the_program() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    let first = tab_rect(&fixture, "todo-1.txt");
    let second = tab_rect(&fixture, "todo-2.txt");
    fixture
        .test
        .drag(second.center(), Pos2::new(first.max.x + 60.0, first.center().y));
    fixture.settle();

    let intercepted = fixture.test.intercepted_keys();
    assert!(
        intercepted.contains(&KeyChord::new(Key::Tab, Modifiers::LOGO)),
        "the desktop asks the host for Super+Tab over the program: {intercepted:?}"
    );
    assert!(
        !intercepted.contains(&KeyChord::new(Key::Tab, Modifiers::CTRL)),
        "and leaves Ctrl+Tab to it"
    );

    let shown = shown_tab(&fixture);
    assert!(fixture.test.host_key(Modifiers::LOGO, Key::Tab));
    fixture.settle();
    let next = shown_tab(&fixture);
    assert_ne!(next, shown, "Super+Tab shows the window's other tab");
    let back = Modifiers {
        shift: true,
        ..Modifiers::LOGO
    };
    assert!(fixture.test.host_key(back, Key::Tab));
    fixture.settle();
    assert_eq!(shown_tab(&fixture), shown, "Super+Shift+Tab walks back");
}
