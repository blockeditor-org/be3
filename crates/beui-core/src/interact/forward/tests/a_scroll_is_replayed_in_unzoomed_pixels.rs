use super::*;

#[test]
fn a_scroll_is_replayed_in_unzoomed_pixels() {
    let mut replay = Replay::default();
    let at = pos2(150.0, 100.0);
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                mouse(PointerPhase::ScrollStarted, at),
                mouse(PointerPhase::Scroll(vec2(-8.0, 40.0)), at),
                mouse(
                    PointerPhase::ScrollEnded {
                        fling: vec2(0.0, 100.0)
                    },
                    at
                ),
                mouse(PointerPhase::Zoom(1.5), at),
            ],
            2.0
        ),
        [
            Input::ScrollStarted,
            Input::Scroll(ScrollDelta::Pixels(vec2(-4.0, 20.0))),
            Input::ScrollEnded,
            Input::Zoom(1.5),
        ]
    );
}
