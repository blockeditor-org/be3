use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_program_window_dragged_to_another_display_shows_on_both_on_the_way() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 160.0), vec2(640.0, 480.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 2.0));
    fixture.show_windows(&[host_window(1, true)]);
    let windows = fixture.test.desk_windows();
    assert_eq!(
        windows.len(),
        1,
        "the program's window is a window of the host's own: {windows:?}"
    );
    assert!(
        left.contains_rect(windows[0]),
        "it opens on the first display"
    );
    assert_eq!(fixture.test.desk_window_scales(), [1.0]);

    let tab = fixture.desk_text("notes-1.txt");
    let grab = pos2(tab.max.x + 80.0, tab.center().y);
    let seam = left.max.x - windows[0].center().x;
    let halfway = grab + vec2(seam, 200.0);
    fixture.test.desk_press(grab);
    for step in 1..=4 {
        fixture
            .test
            .desk_move(grab + (halfway - grab) * (step as f32 / 4.0));
    }
    let moving = fixture.test.desk_windows()[0];
    assert!(
        moving.intersects(left) && moving.intersects(right),
        "half way it straddles the seam: {moving:?}"
    );
    fixture
        .test
        .desk_snapshot("a_program_window_half_way_on_the_first_display", 0);
    fixture
        .test
        .desk_snapshot("a_program_window_half_way_on_the_second_display", 1);

    let there = grab + vec2(seam + 500.0, 240.0);
    for step in 1..=4 {
        fixture
            .test
            .desk_move(halfway + (there - halfway) * (step as f32 / 4.0));
    }
    fixture.test.desk_release(there);
    fixture.settle();
    let placed = fixture.test.desk_windows()[0];
    let shared = |display: Rect| {
        let shared = display.intersect(placed);
        shared.width().max(0.0) * shared.height().max(0.0)
    };
    assert!(
        shared(right) > shared(left),
        "let go, the window is mostly on the second display: {placed:?}"
    );
    assert_eq!(
        fixture.test.desk_window_scales(),
        [2.0],
        "and is drawn at that display's scale"
    );
    fixture
        .test
        .desk_snapshot("a_program_window_moved_to_the_second_display", 1);
}
