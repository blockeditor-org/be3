use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::PointerPress;
use beui::input::{Pen, PenPhase};

#[test]
fn a_pen_presses_and_drags_like_a_mouse_and_reports_its_pressure() {
    let _root = crate::reactive::RootScope::new();
    let presses = create_signal::<Vec<PointerPress>>(Vec::new());
    let hovered = create_signal(false);
    let document = build_node(move || {
        view! {
            <Interactive
                on_hover_change={hovered}
                on_press={move |press: PointerPress| presses.update(|presses| presses.push(press))}
                on_drag={move |press: PointerPress| presses.update(|presses| presses.push(press))}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tilted = |pressure| Pen {
        pressure,
        tilt: vec2(10.0, -5.0),
        eraser: false,
    };
    let pen = |phase, pos, pressure| Input::Pen {
        phase,
        pos,
        pen: tilted(pressure),
    };

    harness.frame(vec![pen(PenPhase::Move, pos2(100.0, 100.0), 0.0)]);
    assert!(
        hovered.get_untracked(),
        "a pen in range hovers what it is over"
    );

    harness.frame(vec![pen(
        PenPhase::Down(PointerButton::Primary),
        pos2(100.0, 100.0),
        0.25,
    )]);
    harness.frame(vec![pen(PenPhase::Move, pos2(110.0, 100.0), 0.75)]);
    harness.frame(vec![pen(
        PenPhase::Up(PointerButton::Primary),
        pos2(110.0, 100.0),
        0.0,
    )]);

    let presses = presses.get_untracked();
    let seen: Vec<(Pos2, bool, Option<Pen>)> = presses
        .iter()
        .map(|press| (press.pos, press.touch, press.pen))
        .collect();
    assert_eq!(
        (seen.first().copied(), seen.last().copied()),
        (
            Some((pos2(100.0, 100.0), false, Some(tilted(0.25)))),
            Some((pos2(110.0, 100.0), false, Some(tilted(0.75)))),
        ),
        "the press and the drag each carry the pen's pressure and tilt where they happened, and the pen is not a finger"
    );

    harness.frame(vec![pen(PenPhase::Gone, pos2(110.0, 100.0), 0.0)]);
    assert!(
        !hovered.get_untracked(),
        "a pen leaving range stops hovering"
    );
}
