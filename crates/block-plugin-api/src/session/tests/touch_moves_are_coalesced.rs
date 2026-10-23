use super::*;

#[test]
fn touch_moves_are_coalesced() {
    let mut session = running_session();
    let touch = |x: f32, y: f32, force: f32, time_micros: u64| {
        timed(
            Input::Touch {
                id: crate::TouchId {
                    device: 2,
                    finger: 3,
                },
                phase: crate::TouchPhase::Move,
                pos: Pos2::new(x, y),
                force: Some(force),
            },
            time_micros,
        )
    };
    session.enqueue(input(touch(10.0, 20.0, 0.4, 1))).unwrap();
    session.enqueue(input(touch(30.0, 40.0, 0.8, 2))).unwrap();

    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(
        session.next_outbound(),
        Some(input(touch(30.0, 40.0, 0.8, 2)))
    );
}
