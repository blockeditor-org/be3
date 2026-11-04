use be_sdk_beui::beui::{pos2, vec2};

use super::*;

fn closes(fixture: &Fixture) -> Vec<String> {
    let mut closes: Vec<String> = fixture
        .test
        .test_ids()
        .iter()
        .map(|(id, _)| id.to_owned())
        .filter(|id| id.starts_with("dock.") && id.ends_with(".close"))
        .filter(|id| !fixture.test.desk_rects_of(id).is_empty())
        .collect();
    closes.sort();
    closes
}

#[test]
fn a_window_holding_one_tab_closes_from_its_tab_and_its_bar() {
    let mut fixture = Fixture::new();
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0));
    fixture.test.show_on_displays(&[(display, 1.0)]);
    fixture.settle();
    fixture.show_windows(&[host_window(1, true)]);
    fixture.settle();
    let shown = closes(&fixture);
    assert_eq!(
        shown.len(),
        2,
        "a window of one tab closes from its tab and from its bar: {shown:?}"
    );
    assert!(
        shown[0].starts_with("dock.tab.") && shown[1].starts_with("dock.window."),
        "one close is the tab's and the other the window's: {shown:?}"
    );
    fixture.test.desk_snapshot(
        "a_window_holding_one_tab_closes_from_its_tab_and_its_bar",
        0,
    );
}
