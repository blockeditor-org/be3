use be_lib_geometry::pos2;
use be_lib_ui_types::input::{Input, PointerButton, encode};

use super::*;

/// The layer the root's frame `done` shows at `id`.
fn layer(done: &FrameDone, id: LayerId) -> Option<be_lib_layers::Layer> {
    LayerList::decode(&done.layers)
        .expect("the frame is a layer list")
        .windows
        .into_iter()
        .flat_map(|window| window.layers)
        .find(|layer| layer.id == id)
}

#[test]
fn input_reaches_the_child_under_it_by_the_layers_drawn_last() {
    let mut root = root();
    let frame = |root: &mut Root, time| {
        root.frame(FrameRequest { time, wall_clock: 0 }, Instant::now() + PATIENT)
            .expect("the root draws")
    };
    let done = frame(&mut root, 1);
    assert_eq!(shown(&done), vec![ROOT_LAYER, CHILD_LAYER]);
    let child = layer(&done, CHILD_LAYER).expect("the child shows");
    assert_eq!(
        child.transform.origin(),
        pos2(CHILD_ORIGIN.0, CHILD_ORIGIN.1)
    );

    root.input(
        None,
        encode(&[Input::PointerMoved(pos2(50.0, 50.0)).into()]),
    )
    .expect("the root takes input");
    let done = frame(&mut root, 2);
    assert_eq!(
        layer(&done, HEARD_LAYER),
        None,
        "a pointer over the root's own surface reaches no child"
    );

    root.input(
        None,
        encode(&[
            Input::PointerButton {
                pos: pos2(150.0, 50.0),
                button: PointerButton::Primary,
                pressed: true,
            }
            .into(),
            // Off the child's screen, and off the window: the press holds the pointer for it.
            Input::PointerMoved(pos2(250.0, 60.0)).into(),
        ]),
    )
    .expect("the root takes input");
    let done = frame(&mut root, 3);
    let heard = layer(&done, HEARD_LAYER).expect("the child heard the press");
    assert_eq!(
        heard.transform.origin(),
        pos2(250.0 - CHILD_ORIGIN.0, 60.0),
        "the child heard where the pointer went while it held the press, in its own coordinates"
    );
}
