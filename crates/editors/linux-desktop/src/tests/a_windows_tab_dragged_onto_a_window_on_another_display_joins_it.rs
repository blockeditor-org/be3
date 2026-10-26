use block_editor_beui::beui::unstyled::DockApi;
use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_windows_tab_dragged_onto_a_window_on_another_display_joins_it() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 0.0), vec2(800.0, 640.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 2.0));
    fixture.show_windows(&[host_window(1, false), host_window(2, true)]);
    assert_eq!(fixture.test.desk_windows().len(), 2, "a window each");

    let second = fixture.desk_text("Program 2");
    let grab = pos2(second.max.x + 80.0, second.center().y);
    fixture.test.desk_drag(
        grab,
        grab + vec2(right.min.x - second.min.x + 40.0, 40.0),
        6,
    );
    fixture.settle();
    let moved = fixture.desk_text("Program 2");
    assert!(
        right.contains_rect(moved),
        "the second window is on the second display: {moved:?}"
    );

    let first = fixture.desk_text("Program 1");
    let onto = pos2(first.max.x + 60.0, first.center().y);
    let tab = moved.center();
    fixture.test.desk_press(tab);
    fixture.test.desk_move(tab + vec2(-20.0, 20.0));
    fixture.test.desk_move(onto + vec2(40.0, 0.0));
    fixture.test.desk_move(onto);
    fixture.test.desk_release(onto);
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
        "both programs are tabs of the window on the other display"
    );
    drop(document);
    assert_eq!(
        fixture.test.desk_windows().len(),
        1,
        "and the host shows one window"
    );
    fixture
        .test
        .desk_snapshot("a_tab_joined_a_window_on_the_other_display", 0);
}
