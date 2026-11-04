use super::*;

#[test]
fn a_wheel_is_replayed_in_its_notches() {
    let mut replay = Replay::default();
    let at = pos2(150.0, 100.0);
    assert_eq!(
        replayed(
            &mut replay,
            vec![mouse(
                PointerPhase::Scroll {
                    delta: vec2(0.0, -80.0),
                    notches: Some(vec2(0.0, -2.0)),
                },
                at
            )],
            2.0
        ),
        [Input::Scroll(ScrollDelta::Lines(vec2(0.0, -2.0)))],
        "the editor scrolls its own lines for a wheel, whatever the zoom"
    );
}
