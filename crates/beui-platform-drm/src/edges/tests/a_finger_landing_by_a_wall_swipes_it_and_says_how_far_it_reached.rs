use super::*;

#[test]
fn a_finger_landing_by_a_wall_swipes_it_and_says_how_far_it_reached() {
    let mut edges = edges(bottom(true, false));

    assert!(
        !edges.touch_down(1, pos2(400.0, 300.0)),
        "a finger in the middle of the display is the program's"
    );
    assert!(!edges.touch_moved(1, pos2(400.0, 290.0)));

    assert!(edges.touch_down(2, pos2(400.0, 595.0)));
    assert_eq!(edges.states()[0].pull, Some(5.0));
    assert!(edges.touch_moved(2, pos2(410.0, 560.0)));
    assert_eq!(edges.states()[0].pull, Some(40.0));
    assert!(edges.touch_up(2, false));
    let state = &edges.states()[0];
    assert_eq!((state.pull, state.pulls, state.pulled), (None, 1, 40.0));

    let mut deliberate = super::edges(bottom(true, true));
    assert!(
        !deliberate.touch_down(3, pos2(400.0, 590.0)),
        "over a full screen program a swipe starts closer to the edge"
    );
    assert!(deliberate.touch_down(4, pos2(400.0, 597.0)));
    assert!(deliberate.touch_up(4, true));
    assert_eq!(
        deliberate.states()[0].pulled,
        0.0,
        "a cancelled swipe reaches nothing"
    );

    edges.set(vec![ScreenEdge {
        keep: Some(Rect::from_min_size(pos2(0.0, 540.0), vec2(800.0, 60.0))),
        ..bottom(false, false)
    }]);
    assert!(
        !edges.touch_down(5, pos2(400.0, 560.0)),
        "a finger on what the edge revealed is the program's"
    );
    assert_eq!(edges.states()[0].taps_away, 0);
    assert!(!edges.touch_down(6, pos2(400.0, 200.0)));
    assert_eq!(
        edges.states()[0].taps_away,
        1,
        "a finger landing away from it says so"
    );
}
