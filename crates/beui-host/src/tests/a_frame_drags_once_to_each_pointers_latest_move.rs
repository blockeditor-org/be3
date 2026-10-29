use super::*;
use crate::input::PointerPress;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_frame_drags_once_to_each_pointers_latest_move() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<f32>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive
                on_drag={move |press: PointerPress| drags.update(|drags| drags.push(press.pos.x))}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    assert!(
        !output.main().window.requests.every_sample,
        "nothing asks for every sample"
    );
    let moved = |x| Input::PointerMoved(pos2(x, 100.0));
    harness.frame(vec![Input::PointerButton {
        pos: pos2(100.0, 100.0),
        button: PointerButton::Primary,
        pressed: true,
    }]);
    harness.frame(vec![moved(120.0)]);
    drags.set(Vec::new());

    harness.frame(vec![moved(130.0), moved(140.0), moved(150.0)]);
    assert_eq!(
        drags.get_untracked(),
        vec![150.0],
        "the moves of one frame drag once, to the latest"
    );

    drags.set(Vec::new());
    harness.frame(vec![
        moved(160.0),
        Input::PointerButton {
            pos: pos2(170.0, 100.0),
            button: PointerButton::Primary,
            pressed: false,
        },
        moved(180.0),
    ]);
    assert_eq!(
        drags.get_untracked(),
        vec![160.0, 170.0],
        "a release keeps the move before it and drags to where it was made"
    );
}
