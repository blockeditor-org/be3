use super::*;

#[test]
fn the_curve_slider_bends_the_panel_and_clicks_land_on_the_curve() {
    let (mut space, now) = space();
    let bend = on_part(&space, WindowId::MAIN, Part::Bend);
    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(bend),
            button(bend, true),
            button(bend, false),
        ],
    );
    assert_eq!(delivered(&sent), [], "the app sees nothing of the button");
    assert_eq!(
        space.chrome.slider,
        Some(WindowId::MAIN),
        "it opens the slider"
    );

    let track = on_part(&space, WindowId::MAIN, Part::Slider);
    let start = *main(&space);
    let right = screen(
        &space,
        start.world(part_pos(&start, Part::Slider, true) + vec2(400.0, 0.0)),
    );
    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(track),
            button(track, true),
            Input::PointerMoved(right),
        ],
    );
    assert_eq!(delivered(&sent), []);
    assert!(
        (main(&space).bend - crate::panel::MOST_BEND).abs() < 1e-4,
        "the far end of the track bends it most"
    );
    send(
        &mut space,
        now,
        [Input::PointerMoved(track), button(track, false)],
    );
    let panel = *main(&space);
    assert!(
        (panel.bend - crate::panel::MOST_BEND / 2.0).abs() < 0.05,
        "the middle of the track bends it half way: {}",
        panel.bend
    );
    assert!(
        close(panel.pose.position, start.pose.position),
        "its middle stays"
    );

    let pos = pos2(60.0, 120.0);
    let at = screen(&space, panel.world(pos));
    let got = delivered(&send(
        &mut space,
        now,
        [Input::PointerMoved(at), button(at, true)],
    ));
    assert_eq!(got.len(), 2);
    for (window, input) in got {
        assert_eq!(window, WindowId::MAIN);
        assert!((pos_of(&input) - pos).length() < 0.5, "{input:?}");
    }
    send(&mut space, now, [button(at, false)]);

    let bend = on_part(&space, WindowId::MAIN, Part::Bend);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(bend),
            button(bend, true),
            button(bend, false),
        ],
    );
    assert_eq!(
        space.chrome.slider, None,
        "the button closes the slider again"
    );
}
