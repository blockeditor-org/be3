use super::*;

#[test]
fn pointer_inputs_keep_their_time() {
    let mut replay = Replay::default();
    let start = crate::timer::now();
    let at = |seconds| start + Duration::from_secs(seconds);
    let mut moved = pointer(PointerId::Mouse, PointerPhase::Move, pos2(100.0, 50.0));
    moved.time = at(3);
    moved.predicted = Some(pos2(110.0, 60.0));
    let mut touched = pointer(
        PointerId::Touch(TouchId {
            device: 0,
            finger: 1,
        }),
        PointerPhase::Down(PRIMARY),
        pos2(100.0, 50.0),
    );
    touched.time = at(5);
    let times: Vec<_> = timed(
        &mut replay,
        vec![
            sent(moved),
            sent(touched),
            mouse(PointerPhase::Scroll(vec2(0.0, 10.0)), pos2(100.0, 50.0)),
            key(Key::A, Modifiers::NONE, None),
        ],
        1.0,
    )
    .into_iter()
    .map(|replayed| replayed.time)
    .collect();
    assert_eq!(times, [Some(at(3)), None, Some(at(5)), None, None]);
}
