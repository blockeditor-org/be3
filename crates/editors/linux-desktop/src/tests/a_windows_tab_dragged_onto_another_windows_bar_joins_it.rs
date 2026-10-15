use block_editor_beui::beui::Pos2;

use super::*;

fn tab_rect(fixture: &Fixture, title: &str) -> Rect {
    let document = fixture.test.document();
    let node = document
        .root()
        .and_then(|root| text_within(document, root, title))
        .unwrap_or_else(|| panic!("{title} has a tab"));
    document.node_rect(node).expect("the tab is laid out")
}

#[test]
fn a_windows_tab_dragged_onto_another_windows_bar_joins_it() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    let first = tab_rect(&fixture, "Program 1");
    let second = tab_rect(&fixture, "Program 2");
    let to = Pos2::new(first.max.x + 60.0, first.center().y);
    fixture.test.drag(second.center(), to);
    fixture.settle();
    let moved = tab_rect(&fixture, "Program 2");
    assert!(
        (moved.center().y - tab_rect(&fixture, "Program 1").center().y).abs() < 1.0,
        "the dragged tab sits in the other window's bar: {moved:?}"
    );
}
