use super::*;

#[test]
fn a_window_asked_to_resize_keeps_its_top_left_corner_and_reports_the_size() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    let corner = space.panel(window).expect("opened").world(Pos2::ZERO);

    space.request(&WindowRequest::Resize {
        window,
        size: vec2(400.0, 300.0),
    });
    let resized = space.panel(window).expect("still open");
    assert_eq!(resized.size, vec2(400.0, 300.0));
    assert!(
        close(resized.world(Pos2::ZERO), corner),
        "the top-left corner stays put"
    );
    assert_eq!(
        reports(&send(&mut space, now, [])),
        [WindowReport::Resized {
            window,
            size: vec2(400.0, 300.0)
        }],
        "the host tells the app the size it gave"
    );
}
