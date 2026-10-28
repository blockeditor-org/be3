use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn dragging_onto_a_drop_target_hands_it_the_payload() {
    let _root = crate::reactive::RootScope::new();
    let dropped = create_signal(None::<u32>);
    let clicks = create_signal(0);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable
                    payload={7_u32}
                    preview={|_: u32| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                >|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, _): (u32, DragPoint)| dropped.set(Some(payload))}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(source));
    assert_eq!(
        clicks.get_untracked(),
        1,
        "a press that never moved is a click"
    );
    assert_eq!(dropped.get_untracked(), None);

    harness.drag(harness.center(source), harness.center(target));
    assert_eq!(
        dropped.get_untracked(),
        Some(7),
        "the target is handed what was dragged"
    );
    assert_eq!(clicks.get_untracked(), 1, "a drag is not also a click");
}
