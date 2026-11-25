use super::*;

#[test]
fn a_window_placed_and_resized_is_asked_for_where_it_went_and_at_its_new_size() {
    let mut windows = Vec::new();
    let _ = hear(
        &mut windows,
        &[main_screen(true)],
        vec![open(7, 1, pos2(40.0, 30.0))],
        None,
        Pos2::ZERO,
    );
    assert!(
        hear(
            &mut windows,
            &[main_screen(true)],
            vec![
                WindowRequest::Place(WindowPlace {
                    view: ViewId(7),
                    placement: WindowPlacement::Over(WindowOver {
                        view: ViewRef {
                            instance: EDITOR,
                            view: ViewId::MAIN,
                        },
                        at: pos2(100.0, 80.0),
                    }),
                }),
                WindowRequest::Resize(WindowResize {
                    view: ViewId(7),
                    size: vec2(300.0, 150.0),
                }),
            ],
            None,
            Pos2::ZERO,
        )
        .0
    );
    let request = windows[0].request();
    assert_eq!(
        request.anchor_space.map(|space| space.to_host.origin),
        Some(pos2(100.0, 80.0)),
        "it anchors its views from where it went"
    );
    assert_eq!(request.metrics.logical, vec2(300.0, 150.0));
    assert_eq!(
        (request.metrics.pixel_width, request.metrics.pixel_height),
        (600, 300),
        "laid out at its new size, at its opener's pixels per point"
    );
}
