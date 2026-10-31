use block_editor_beui::beui::Pos2;
use block_editor_beui::beui::dock::styled::DockApi;

use super::*;

fn tab_rect(fixture: &Fixture, title: &str) -> Rect {
    let document = fixture.test.document();
    let node = document
        .root()
        .and_then(|root| text_within(&document, root, title))
        .unwrap_or_else(|| panic!("{title} has a tab"));
    document.node_rect(node).expect("the tab is laid out")
}

#[test]
fn a_windows_tab_dragged_onto_another_windows_bar_joins_it() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    let first = tab_rect(&fixture, "todo-1.txt");
    let second = tab_rect(&fixture, "todo-2.txt");
    let to = Pos2::new(first.max.x + 60.0, first.center().y);
    fixture.test.drag(second.center(), to);
    fixture.settle();
    let document = fixture.test.document();
    let dock = document
        .find_test_id("desktop.dock")
        .expect("the desktop has a dock");
    let state = document.api::<DockApi>(dock).state();
    let windows = state.windows();
    assert_eq!(windows.len(), 1, "the dragged tab left its own window");
    assert_eq!(
        state.surface_tabs(windows[0]).len(),
        2,
        "both programs are tabs of the window the tab was dropped on"
    );
}
