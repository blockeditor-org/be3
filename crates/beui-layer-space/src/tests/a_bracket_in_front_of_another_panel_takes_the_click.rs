use super::*;

#[test]
fn a_bracket_in_front_of_another_panel_takes_the_click() {
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
    let out = (chrome::GAP + chrome::THICKNESS / 2.0) * panel.chrome.scale / panel.metres_per_point;
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
        got.len() == 2 && got[1] == (WindowId::MAIN, Input::PointerLeft),
        "the main window under the bracket only sees the cursor leave: {got:?}"
    );
    assert_eq!(
        space.mouse.gesture.map(|gesture| gesture.part()),
        Some(Part::Scale(Corner::TopLeft)),
        "the nearer bracket takes the press"
    );
    assert_eq!(space.focused(), Some(over), "and focuses its panel");

    let further = bracket - vec2(40.0, 30.0);
    let sent = send(
        &mut space,
        now,
        [Input::PointerMoved(further), button(further, false)],
    );
    assert_eq!(delivered(&sent), []);
    assert_eq!(placed(&sent).len(), 1);
    assert!(
        space.panel(over).unwrap().metres_per_point > panel.metres_per_point,
        "dragging the top-left bracket out scales the panel up"
    );
}
