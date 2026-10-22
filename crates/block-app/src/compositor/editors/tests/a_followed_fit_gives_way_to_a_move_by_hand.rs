use super::*;

#[test]
fn a_followed_fit_gives_way_to_a_move_by_hand() {
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let follow = |resume| Command::Follow {
        factor: 2.0,
        anchor: rect.center(),
        delta: vec2(10.0, 0.0),
        resume,
    };
    let mut viewport = Viewport::default();

    settle(&mut viewport, follow(false), rect, MAX_ZOOM);
    assert_eq!((viewport.zoom, viewport.pan), (2.0, vec2(10.0, 0.0)));
    assert!(
        !viewport.moved,
        "a fit the editor keeps is not a move by hand"
    );

    settle(&mut viewport, Command::Pan(vec2(0.0, 5.0)), rect, MAX_ZOOM);
    assert!(
        viewport.moved,
        "a pan the person made moves the view by hand"
    );
    let moved = viewport;
    settle(&mut viewport, follow(false), rect, MAX_ZOOM);
    assert_eq!(
        viewport, moved,
        "a fit asked for before the editor saw the move leaves the view where the person put it"
    );

    settle(&mut viewport, follow(true), rect, MAX_ZOOM);
    assert_eq!(viewport.zoom, 4.0);
    assert!(
        !viewport.moved,
        "a Fit the person pressed follows the content again"
    );
}
