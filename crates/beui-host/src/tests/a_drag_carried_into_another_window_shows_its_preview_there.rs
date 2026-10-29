use super::*;
use crate::WindowId;
use crate::unstyled::{
    DRAG_PREVIEW_OFFSET, DragHandle, DragPoint, Draggable, DropHandle, DropTarget,
};
use crate::{Window, WindowHandle};

#[test]
fn a_drag_carried_into_another_window_shows_its_preview_there() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let preview = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Draggable
                    payload={7_u32}
                    preview={move |_: u32| view! {
                        <Frame @node_ref={&preview} width=30.0 height=10.0 />
                    }}
                >|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
                <Window id={window} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <DropTarget on_drop={move |_: (u32, DragPoint)| {}}>|_: DropHandle|
                        <Frame />
                    </DropTarget>
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    harness.frame(vec![
        Input::PointerMoved(pos2(20.0, 20.0)),
        Input::PointerButton {
            pos: pos2(20.0, 20.0),
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.frame(vec![Input::PointerMoved(pos2(30.0, 60.0))]);
    let shown = |harness: &Harness| {
        let node = preview.get();
        let document = harness.document();
        (
            document.window_of(node),
            document.node_rect(node).map(|rect| rect.min),
        )
    };
    assert_eq!(
        shown(&harness),
        (WindowId::MAIN, Some(pos2(30.0, 60.0) + DRAG_PREVIEW_OFFSET)),
        "the preview follows the pointer in the window the drag began in"
    );

    harness.frame_in(window, vec![Input::PointerMoved(pos2(70.0, 60.0))]);
    harness.frame_in(window, vec![Input::PointerMoved(pos2(80.0, 70.0))]);
    assert_eq!(
        shown(&harness),
        (window, Some(pos2(80.0, 70.0) + DRAG_PREVIEW_OFFSET)),
        "carried into another window, the preview is shown in that window, under the pointer"
    );
}
