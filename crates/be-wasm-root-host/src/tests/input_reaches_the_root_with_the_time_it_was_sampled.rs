use be_lib_geometry::pos2;
use be_lib_ui_types::input::{Input, PointerButton, Timed, encode};
use be_wasm_root_testing::TIMED_LAYER;

use super::*;

/// The time, in microseconds, the frame `done`'s timed marker says the child last heard input
/// at.
fn timed(done: &FrameDone) -> Option<f32> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .into_iter()
        .flat_map(|window| window.layers)
        .find(|layer| layer.id == TIMED_LAYER)
        .map(|layer| layer.transform.origin().x)
}

#[test]
fn input_reaches_the_root_with_the_time_it_was_sampled() {
    let mut root = root();
    let frame = |root: &mut Root, time| {
        root.frame(FrameRequest { time, wall_clock: 0 }, Instant::now() + PATIENT)
            .expect("the root draws")
    };
    frame(&mut root, 1);
    let on_child = pos2(CHILD_ORIGIN.0 + 10.0, 50.0);
    root.input(
        None,
        encode(&[
            Timed {
                input: Input::PointerButton {
                    pos: on_child,
                    button: PointerButton::Primary,
                    pressed: true,
                },
                time: Some(1_500_000),
            },
            Timed {
                input: Input::PointerMoved(pos2(on_child.x + 5.0, 50.0)),
                time: Some(2_250_000),
            },
            // Untimed, so the last time the child heard stays the move's.
            Input::PointerButton {
                pos: on_child,
                button: PointerButton::Primary,
                pressed: false,
            }
            .into(),
        ]),
    )
    .expect("the root takes input");
    let done = frame(&mut root, 2);
    assert_eq!(
        timed(&done),
        Some(2_250.0),
        "the root heard each input with the time its host sampled it at"
    );
}
