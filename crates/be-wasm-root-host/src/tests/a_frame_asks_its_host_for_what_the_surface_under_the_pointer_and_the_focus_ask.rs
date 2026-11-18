use be_lib_geometry::pos2;
use be_lib_ui_types::input::{Input, PointerButton, encode};
use be_wasm_root_testing::CHILD_CURSOR;

use super::*;

#[test]
fn a_frame_asks_its_host_for_what_the_surface_under_the_pointer_and_the_focus_ask() {
    let mut root = root();
    let frame = |root: &mut Root, time| {
        root.frame(FrameRequest { time }, Instant::now() + PATIENT)
            .expect("the root draws")
    };
    let done = frame(&mut root, 1);
    assert!(
        done.every_sample,
        "the child it shows wants every sample of the pointer's moves"
    );
    assert_eq!(done.cursor, "default", "the pointer is over nothing yet");
    assert!(!done.pointer_locked);

    let on_child = pos2(CHILD_ORIGIN.0 + 10.0, 50.0);
    root.input(None, encode(&[Input::PointerMoved(on_child).into()]))
        .expect("the root takes input");
    let done = frame(&mut root, 2);
    assert_eq!(
        done.cursor, CHILD_CURSOR,
        "over the child's screen, the window shows its cursor"
    );
    assert!(!done.pointer_locked, "the child has no focus yet");

    root.input(
        None,
        encode(&[
            Input::PointerButton {
                pos: on_child,
                button: PointerButton::Primary,
                pressed: true,
            }
            .into(),
            Input::PointerButton {
                pos: on_child,
                button: PointerButton::Primary,
                pressed: false,
            }
            .into(),
        ]),
    )
    .expect("the root takes input");
    let done = frame(&mut root, 3);
    assert!(
        done.pointer_locked,
        "the press focused the child, which holds the pointer lock"
    );

    root.input(None, encode(&[Input::PointerMoved(pos2(10.0, 50.0)).into()]))
        .expect("the root takes input");
    let done = frame(&mut root, 4);
    assert_eq!(
        done.cursor, "default",
        "over the root's own surface, the default cursor"
    );
}
