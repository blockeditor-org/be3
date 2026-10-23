use super::*;

#[test]
fn touch_phases_become_touch_inputs() {
    let id = TouchId {
        device: 3,
        finger: 7,
    };
    let mut replay = Replay::default();
    let touched = |phase| {
        let mut event = pointer(PointerId::Touch(id), phase, pos2(120.0, 70.0));
        event.force = Some(0.25);
        sent(event)
    };
    let phases = [
        (PointerPhase::Down(PRIMARY), TouchPhase::Start),
        (PointerPhase::Move, TouchPhase::Move),
        (PointerPhase::Up(PRIMARY), TouchPhase::End),
        (PointerPhase::Cancel, TouchPhase::Cancel),
    ];
    assert_eq!(
        replayed(
            &mut replay,
            phases.iter().map(|(phase, _)| touched(*phase)).collect(),
            2.0
        ),
        phases
            .iter()
            .map(|(_, phase)| Input::Touch {
                id,
                phase: *phase,
                pos: pos2(10.0, 10.0),
                force: Some(0.25),
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        replayed(&mut replay, vec![touched(PointerPhase::Gone)], 2.0),
        [],
        "a touch has no hover to be gone from"
    );
}
