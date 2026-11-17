use super::*;

#[test]
fn left_or_right_an_overlay_fitting_neither_side_keeps_its_width_and_shifts_over_its_anchor() {
    let shown = placed(
        Placement::RIGHT_START,
        rect(100.0, 120.0, 300.0, 150.0),
        150.0,
        40.0,
    );
    assert_eq!(
        shown,
        Rect::from_min_size(pos2(250.0, 120.0), Vec2::new(150.0, 40.0)),
        "with as much room on each side it keeps to its own, shifted in over its anchor"
    );
    let shown = placed(
        Placement::LEFT_START,
        rect(50.0, 120.0, 300.0, 150.0),
        150.0,
        40.0,
    );
    assert_eq!(
        shown,
        Rect::from_min_size(pos2(250.0, 120.0), Vec2::new(150.0, 40.0)),
        "left takes the roomier right"
    );
    let narrow = rect(0.0, 0.0, 350.0, 300.0);
    let row = rect(0.0, 40.0, 200.0, 70.0);
    let submenu = Vec2::new(200.0, 120.0);
    let shown = resolve_rect(narrow, row, Placement::RIGHT_START, submenu);
    assert_eq!(
        shown,
        Rect::from_min_size(pos2(150.0, 40.0), submenu),
        "a submenu in a narrow window keeps its rows whole, over the end of its row"
    );
    assert_eq!(
        room(narrow, row, Placement::RIGHT_START),
        narrow.size(),
        "and its room is the window's"
    );
}
