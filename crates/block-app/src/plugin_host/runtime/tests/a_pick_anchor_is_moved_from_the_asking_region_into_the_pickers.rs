use super::*;

#[test]
fn a_pick_anchor_is_moved_from_the_asking_region_into_the_pickers() {
    let window = FramePlacement {
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 800.0)),
        zoom: 1.0,
    };
    let tab = FramePlacement {
        rect: Rect::from_min_size(pos2(260.0, 40.0), vec2(940.0, 760.0)),
        zoom: 1.0,
    };
    let button = Rect::from_min_size(Pos2::new(12.0, 8.0), Vec2::new(28.0, 28.0));
    assert_eq!(
        reanchor(button, tab, window),
        Some(Rect::from_min_size(
            Pos2::new(272.0, 48.0),
            Vec2::new(28.0, 28.0)
        )),
        "a tab's button lands where the tab sits in the window"
    );

    let zoomed = FramePlacement {
        rect: Rect::from_min_size(pos2(400.0, 300.0), vec2(200.0, 100.0)),
        zoom: 2.0,
    };
    assert_eq!(
        reanchor(button, zoomed, window),
        Some(Rect::from_min_size(
            Pos2::new(424.0, 316.0),
            Vec2::new(56.0, 56.0)
        )),
        "a block shown zoomed in a canvas anchors at the size it is drawn"
    );

    let unplaced = FramePlacement {
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(0.0, 0.0)),
        zoom: f32::NAN,
    };
    assert_eq!(reanchor(button, unplaced, window), None);
}
