use super::*;
use crate::input::{Pen, PenPhase, PointerPress};
use crate::primitives::*;
use crate::reactive::*;
use std::time::{Duration, Instant};

#[test]
fn every_sample_drags_at_the_time_it_happened() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<PointerPress>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive
                every_sample=true
                on_drag={move |press: PointerPress| drags.update(|drags| drags.push(press))}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    assert!(
        output.main().window.requests.every_sample,
        "the window asks for every sample"
    );
    let pen = |phase, x| Input::Pen {
        phase,
        pos: pos2(x, 100.0),
        pen: Pen {
            pressure: 0.5,
            ..Pen::default()
        },
    };
    harness.frame(vec![pen(PenPhase::Down(PointerButton::Primary), 100.0)]);
    drags.set(Vec::new());

    let before = harness.context().now();
    let early = before + Duration::from_millis(3);
    let later = before + Duration::from_millis(9);
    for incoming in [
        Incoming::points(pen(PenPhase::Move, 110.0)),
        Incoming::Time(early),
        Incoming::points(pen(PenPhase::Move, 120.0)),
        Incoming::Time(later),
        Incoming::points(Input::Predicted(pos2(135.0, 100.0))),
        Incoming::points(pen(PenPhase::Move, 130.0)),
    ] {
        harness.runner().push(incoming);
    }
    harness.frame(Vec::new());
    let now = harness.context().now();
    let seen: Vec<(f32, Instant, Option<Pos2>)> = drags
        .get_untracked()
        .iter()
        .map(|press| (press.pos.x, press.time, press.predicted))
        .collect();
    assert_eq!(
        seen,
        vec![
            (110.0, early, None),
            (120.0, later, Some(pos2(135.0, 100.0))),
            (130.0, now, None),
        ],
        "a stamped sample drags at its own time, its prediction still reaches it, and an unstamped one happens at the frame's"
    );
}
