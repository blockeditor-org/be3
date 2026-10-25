use super::*;

#[test]
fn a_nearer_panel_is_drawn_at_more_pixels_per_point() {
    let now = Instant::now();
    let mut space = Space::new();
    let mut incoming = Vec::new();
    space.input(&mut incoming, &view(now));
    assert_eq!(
        scaled(&incoming),
        [(WindowId::MAIN, 1.0)],
        "the main panel fills 80% of the view, so with some room to spare it is drawn \
         at the platform's own pixels per point"
    );

    let near = open_ahead(&mut space, now, 0.3);
    let far = open_ahead(&mut space, now, 4.0);
    let near_scale = space.scale(near).expect("the near panel has a scale");
    let far_scale = space.scale(far).expect("the far panel has a scale");
    assert!(
        near_scale > 2.0 && far_scale < 0.5 + 1e-6,
        "a panel 30 cm away is drawn at {near_scale} pixels per point, one 4 m away at {far_scale}"
    );
    let shown = |scale: f32, distance: f32| {
        let focal = PLATFORM.y / 2.0 / (crate::head::FOV_Y / 2.0).tan();
        scale / (focal / distance / crate::panel::POINTS_PER_METRE)
    };
    assert!(
        (1.0..1.5).contains(&shown(near_scale, 0.3)),
        "each texel of a near panel covers a little less than a pixel on screen"
    );
}
