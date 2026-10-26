use super::*;

#[test]
fn a_panel_keeps_its_pixels_per_point_until_its_size_on_screen_changes_a_step() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 1.0);
    let held = space.scale(window).expect("the panel has a scale");
    let rescaled = |sent: &[Incoming]| {
        scaled(sent)
            .into_iter()
            .filter(|(scaled, _)| *scaled == window)
            .collect::<Vec<_>>()
    };

    space.head.position.z -= 0.02;
    let sent = send(&mut space, now, []);
    assert!(
        rescaled(&sent).is_empty(),
        "stepping a little nearer keeps the panel's pixels per point, so nothing is drawn again"
    );
    space.head.position.z += 0.1;
    let sent = send(&mut space, now, []);
    assert!(
        rescaled(&sent).is_empty(),
        "nor does stepping a little back: it changes only once the panel shrinks two steps"
    );

    space.head.position.z -= 0.6;
    send(&mut space, now, []);
    let sent = send(&mut space, later(now, 1.0), []);
    let nearer = space.scale(window).expect("the panel has a scale");
    assert!(nearer > held);
    assert!(
        rescaled(&sent) == [(window, nearer)],
        "walking up to the panel reports its new pixels per point to the app"
    );

    space.head.yaw = std::f32::consts::PI;
    space.head.position.z += 5.0;
    let sent = send(&mut space, now, []);
    assert!(
        rescaled(&sent).is_empty(),
        "a panel out of sight keeps the pixels per point it was drawn at"
    );

    space.close(window);
    assert_eq!(space.scale(window), None);
}
