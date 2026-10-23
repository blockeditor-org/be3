use super::*;

#[test]
fn pen_phases_become_pen_inputs() {
    let tilted = Pen {
        pressure: 0.75,
        tilt: vec2(0.5, -0.25),
        eraser: true,
    };
    let mut replay = Replay::default();
    let penned = |phase, pen| {
        let mut event = pointer(PointerId::Pen, phase, pos2(100.0, 50.0));
        event.pen = pen;
        sent(event)
    };
    let phases = [
        (PointerPhase::Move, PenPhase::Move),
        (PointerPhase::Down(PRIMARY), PenPhase::Down(PRIMARY)),
        (PointerPhase::Up(PRIMARY), PenPhase::Up(PRIMARY)),
        (PointerPhase::Cancel, PenPhase::Gone),
        (PointerPhase::Gone, PenPhase::Gone),
    ];
    assert_eq!(
        replayed(
            &mut replay,
            phases
                .iter()
                .map(|(phase, _)| penned(*phase, Some(tilted)))
                .collect(),
            1.0
        ),
        phases
            .iter()
            .map(|(_, phase)| Input::Pen {
                phase: *phase,
                pos: Pos2::ZERO,
                pen: tilted,
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        replayed(&mut replay, vec![penned(PointerPhase::Move, None)], 1.0),
        [Input::Pen {
            phase: PenPhase::Move,
            pos: Pos2::ZERO,
            pen: Pen::default(),
        }],
        "a pen sample that carries no pen replays one at rest, with no pressure"
    );
}
