use super::*;
use crate::WindowId;
use crate::primitives::{Window, WindowHandle};
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn a_press_in_one_window_dragged_into_another_drops_on_a_target_there() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let dropped = create_signal(None::<(u32, DragPoint)>);
    let beneath = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Draggable payload={7_u32}>|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
                <DropTarget on_drop={move |_: (u32, DragPoint)| beneath.set(true)}>|_: DropHandle|
                    <Frame width=200.0 height=200.0 />
                </DropTarget>
                <Window id={window} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <DropTarget
                        on_drop={move |(payload, point): (u32, DragPoint)| dropped.set(Some((payload, point)))}
                    >|_: DropHandle|
                        <Frame />
                    </DropTarget>
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let press = |pressed| Input::PointerButton {
        pos: pos2(20.0, 20.0),
        button: PointerButton::Primary,
        pressed,
    };
    harness.frame(vec![Input::PointerMoved(pos2(20.0, 20.0)), press(true)]);
    harness.frame(vec![Input::PointerMoved(pos2(30.0, 60.0))]);
    harness.frame(vec![Input::PointerMoved(pos2(30.0, 100.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(70.0, 60.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(80.0, 70.0))]);
    harness.frame_in(
        window,
        vec![Input::PointerButton {
            pos: pos2(80.0, 70.0),
            button: PointerButton::Primary,
            pressed: false,
        }],
    );

    let (payload, point) = dropped
        .get_untracked()
        .expect("the drop lands on the target in the window the pointer is over");
    assert_eq!(payload, 7);
    assert_eq!(
        point.window, window,
        "the drop says which window it landed in"
    );
    assert_eq!(
        point.pos,
        pos2(80.0, 70.0),
        "at the pointer's place in that window"
    );
    assert!(
        !beneath.get_untracked(),
        "the target in the main window under the same coordinates takes nothing"
    );
}
