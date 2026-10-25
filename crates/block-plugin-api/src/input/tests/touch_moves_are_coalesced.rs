use super::*;

#[test]
fn touch_moves_are_coalesced() {
    let mut queued = Vec::new();
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
    InputBatch::queue(&mut queued, input(touch(10.0, 20.0, 0.4, 1)));
    InputBatch::queue(&mut queued, input(touch(30.0, 40.0, 0.8, 2)));

    assert_eq!(queued.len(), 1);
    assert_eq!(
        queued.first().cloned(),
        Some(input(touch(30.0, 40.0, 0.8, 2)))
    );
}
