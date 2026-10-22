use super::*;

#[test]
fn a_follow_lands_on_one_view_however_often_it_arrives() {
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let follow = Command::Follow {
        scale: 1.0,
        world: vec2(200.0, 150.0),
        at: rect.center(),
        resume: false,
    };
    let mut once = Viewport::default();
    settle(&mut once, follow, rect, rect.size(), MAX_ZOOM);

    let mut twice = Viewport {
        pan: vec2(-85.5, 0.0),
        ..Viewport::default()
    };
    settle(&mut twice, follow, rect, rect.size(), MAX_ZOOM);
    settle(&mut twice, follow, rect, rect.size(), MAX_ZOOM);
    assert_eq!(
        (twice.zoom, twice.pan),
        (once.zoom, once.pan),
        "a follow worked out from a view the editor had not yet been sent lands where one sent after it would"
    );
}
