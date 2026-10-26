use super::*;

#[test]
fn closing_a_window_removes_its_panel() {
    let (mut space, now) = space();
    let window = open_ahead(&mut space, now, 0.5);
    send(&mut space, now, [Input::PointerMoved(centre())]);

    space.request(&WindowRequest::Close(window));
    assert!(space.panel(window).is_none(), "its panel is gone");
    assert_eq!(space.panels.len(), 1);
    let got = delivered(&send(&mut space, now, [Input::PointerMoved(centre())]));
    assert_eq!(
        got.iter().map(|(window, _)| *window).collect::<Vec<_>>(),
        [WindowId::MAIN],
        "the pointer reaches the main window behind it"
    );

    space.request(&WindowRequest::Close(WindowId::MAIN));
    assert!(
        space.panel(WindowId::MAIN).is_some(),
        "the main window stays"
    );
}
