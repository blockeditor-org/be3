use super::*;

/// The HTML core's twin is crates/beui-demo-html/src/drive/placements.drive.
#[test]
fn near_the_windows_corner_a_placement_flips_and_shifts_clear_of_its_edges() {
    let mut test = alone(WIDE, Page::Layering);
    reveal(&mut test, "demo.placements.corner.trigger");
    test.click("demo.placements.corner.trigger");
    test.frame(Vec::new());
    test.settle();
    let marker = test.rect_of("demo.placements.corner");
    assert_eq!(
        marker,
        beui::Rect::from_min_size(pos2(1020.0, 760.0), Vec2::new(40.0, 20.0)),
        "the marker stands near the bottom right corner"
    );
    let below = test.rect_of("demo.corner.below");
    assert_eq!(below.bottom(), marker.top() - 4.0, "below flips above its anchor");
    assert_eq!(
        below.right(),
        WIDE.x - 8.0,
        "and shifts left, its margin clear of the window's edge"
    );
    let right = test.rect_of("demo.corner.right");
    assert_eq!(right.right(), marker.left() - 4.0, "right flips left of its anchor");
    assert_eq!(right.top(), marker.top(), "lined up with its start");
    test.snapshot("near_the_windows_corner_a_placement_flips_and_shifts_clear_of_its_edges");
}
