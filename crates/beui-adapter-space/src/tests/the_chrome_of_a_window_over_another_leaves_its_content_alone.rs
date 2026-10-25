use super::*;

#[test]
fn the_chrome_of_a_window_over_another_leaves_its_content_alone() {
    let (mut space, now) = space();
    let (over, _) = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(200.0, 200.0),
        },
    );
    let panel = *space.panel(over).unwrap();
    let out = (chrome::GAP + chrome::THICKNESS / 2.0) / panel.metres_per_point;
    let bracket = screen(&space, panel.world(pos2(-out, -out)));
    let aside = screen(&space, main(&space).world(pos2(60.0, 60.0)));
    let got = delivered(&send(
        &mut space,
        now,
        [
            Input::PointerMoved(aside),
            Input::PointerMoved(bracket),
            button(bracket, true),
        ],
    ));
    assert!(
        got.iter().all(|(window, _)| *window == WindowId::MAIN) && got.len() == 3,
        "the main window's content under the bracket gets the press: {got:?}"
    );
    send(&mut space, now, [button(bracket, false)]);

    let inside = screen(&space, panel.world(pos2(20.0, 20.0)));
    send(
        &mut space,
        now,
        [Input::PointerMoved(inside), Input::PointerMoved(bracket)],
    );
    assert_eq!(
        space.chrome.hovered,
        Some((over, Part::Scale(Corner::TopLeft))),
        "coming from its own window, the bracket is in reach"
    );
}
