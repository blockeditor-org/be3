use super::*;

#[test]
fn surface_damage_reaches_the_screen_through_the_quad_it_is_shown_on() {
    let placement = ScreenPlacement {
        screen: ScreenId(1),
        instance: EditorInstanceId(1),
        region: EditorRegion::Frame,
        x: 100,
        y: 50,
        width: 200,
        height: 100,
        scale_factor_millis: 1000,
    };
    let quad = Quad::upright(Rect::from_min_size(pos2(10.0, 20.0), vec2(400.0, 200.0)));
    let whole = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    let rect = SurfaceRect {
        x: 150,
        y: 75,
        width: 20,
        height: 10,
    };

    assert_eq!(
        shown_damage(rect, placement, quad, whole),
        Some(Rect::from_min_size(pos2(110.0, 70.0), vec2(40.0, 20.0)).expand(1.0)),
    );

    let right_half = Rect::from_min_max(pos2(0.5, 0.0), pos2(1.0, 1.0));
    let cropped = Quad::upright(Rect::from_min_size(pos2(10.0, 20.0), vec2(200.0, 200.0)));
    assert_eq!(
        shown_damage(rect, placement, cropped, right_half),
        None,
        "damage on the half of the screen the quad leaves out is not shown",
    );

    let elsewhere = SurfaceRect {
        x: 0,
        y: 0,
        width: 50,
        height: 50,
    };
    assert_eq!(shown_damage(elsewhere, placement, quad, whole), None);
}
