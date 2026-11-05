use super::*;

#[test]
fn a_wall_between_displays_stops_the_pointer_until_it_opens() {
    let outputs = [display(), below()];
    let mut edges = edges(bottom(true, false));

    let at = edges.moved(pos2(400.0, 590.0), vec2(0.0, 50.0), 0, &outputs);
    assert_eq!(
        at,
        pos2(400.0, 599.0),
        "a hidden bar's edge walls the way to the display below"
    );

    edges.set(vec![ScreenEdge {
        keep: Some(Rect::from_min_size(pos2(0.0, 540.0), vec2(800.0, 60.0))),
        ..bottom(false, false)
    }]);
    let at = edges.moved(at, vec2(0.0, 50.0), 8 * MS, &outputs);
    assert_eq!(
        at,
        pos2(400.0, 649.0),
        "once open, the pointer crosses onto it"
    );
}
