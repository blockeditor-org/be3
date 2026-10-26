use super::*;

#[test]
fn pointers_are_replayed_from_the_region_corner_and_unzoomed() {
    let mut replay = Replay::default();
    assert_eq!(
        replayed(
            &mut replay,
            vec![mouse(PointerPhase::Move, pos2(140.0, 70.0))],
            2.0
        ),
        [Input::PointerMoved(pos2(20.0, 10.0))]
    );
    for zoom in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            replayed(
                &mut replay,
                vec![mouse(PointerPhase::Move, pos2(140.0, 70.0))],
                zoom
            ),
            [Input::PointerMoved(pos2(40.0, 20.0))],
            "a zoom of {zoom} replays unzoomed"
        );
    }
    assert_eq!(
        replayed(
            &mut replay,
            vec![
                mouse(PointerPhase::Down(PRIMARY), pos2(110.0, 60.0)),
                mouse(PointerPhase::Up(PRIMARY), pos2(120.0, 90.0)),
                Forwarded::Left,
            ],
            2.0
        ),
        [
            button(pos2(5.0, 5.0), true),
            button(pos2(10.0, 20.0), false),
            Input::PointerLeft,
        ]
    );
}
