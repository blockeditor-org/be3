use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn escape_cancels_a_drag_so_nothing_drops() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<u32>);
    let over = create_signal(false);
    let cancelled = create_signal(0);
    let dragging = create_signal(false);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable
                    payload={7_u32}
                    on_cancel={move |()| cancelled.update(|count| *count += 1)}
                    on_drag_change={move |now: bool| dragging.set(now)}
                >|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
            },
            view! {
                <DropTarget
                    on_over={move |point: Option<(u32, DragPoint)>| over.set(point.is_some())}
                    on_drop={move |(payload, _): (u32, DragPoint)| dropped.set(Some(payload))}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let to = harness.center(target);
    harness.press_at(harness.center(source));
    harness.frame(vec![Input::PointerMoved(to)]);
    assert!(over.get_untracked(), "the drag is over the target");
    assert!(dragging.get_untracked());

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!harness.document().dragging(), "Escape lets go of the drag");
    assert!(!over.get_untracked(), "the target sees it leave");
    assert!(!dragging.get_untracked(), "the source sees it end");
    assert_eq!(cancelled.get_untracked(), 1, "and hears it was cancelled");

    harness.frame(vec![Input::PointerMoved(to + Vec2::new(5.0, 5.0))]);
    assert!(
        !harness.document().dragging(),
        "moving on with the button held does not lift it again"
    );
    harness.release_at(to);
    assert_eq!(dropped.get_untracked(), None, "nothing drops");
    assert_eq!(cancelled.get_untracked(), 1);
}
