use super::*;
use crate::input::{Pen, PenPhase, PointerPress};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn every_coalesced_sample_drags_and_the_last_carries_its_prediction() {
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
    harness.frame(Vec::new());
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
    harness.frame(vec![
        pen(PenPhase::Move, 110.0),
        pen(PenPhase::Move, 120.0),
        pen(PenPhase::Move, 130.0),
        Input::Predicted(pos2(145.0, 100.0)),
    ]);
    let seen: Vec<(f32, Option<Pos2>)> = drags
        .get_untracked()
        .iter()
        .map(|press| (press.pos.x, press.predicted))
        .collect();
    assert_eq!(
        seen,
        vec![
            (110.0, None),
            (120.0, None),
            (130.0, Some(pos2(145.0, 100.0))),
        ],
        "every sample a frame delivers is a drag of its own, and only the latest knows where the pen is predicted to go"
    );

    drags.set(Vec::new());
    harness.frame(vec![pen(PenPhase::Move, 140.0)]);
    harness.frame(vec![pen(PenPhase::Up(PointerButton::Primary), 150.0)]);
    let seen: Vec<Option<Pos2>> = drags
        .get_untracked()
        .iter()
        .map(|press| press.predicted)
        .collect();
    assert_eq!(
        seen,
        vec![None, None],
        "a prediction lasts only until the next sample, and the release has none"
    );
}
