use super::*;

#[test]
fn above_or_below_an_overlay_fitting_neither_side_takes_the_roomier_and_shrinks() {
    let shown = placed(Placement::BELOW_START, rect(150.0, 100.0, 250.0, 200.0), 60.0, 150.0);
    assert_eq!(
        shown,
        Rect::from_min_size(pos2(150.0, 200.0), Vec2::new(60.0, 100.0)),
        "with as much room on each side it keeps to its own, shrunk to fit"
    );
    let shown = placed(Placement::ABOVE_START, rect(150.0, 80.0, 250.0, 120.0), 60.0, 200.0);
    assert_eq!(
        shown,
        Rect::from_min_size(pos2(150.0, 120.0), Vec2::new(60.0, 180.0)),
        "above takes the roomier side below"
    );
    let anchor = rect(150.0, 0.0, 250.0, 300.0);
    let shown = placed(Placement::BELOW_START, anchor, 60.0, 40.0);
    assert_eq!(
        shown.size(),
        Vec2::new(60.0, 40.0),
        "an anchor that leaves no room either side is covered rather than shrunk away"
    );
    assert!(VIEWPORT.contains_rect(shown));
}
