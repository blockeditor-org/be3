use super::*;
use crate::reactive::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn the_innermost_drop_target_that_accepts_the_payload_takes_the_drop() {
    let _root = crate::reactive::RootScope::new();
    let landed = create_signal(Vec::<(&str, u32)>::new());
    let (document, [first, second, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable payload={1_u32}>|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
            },
            view! {
                <Draggable payload={2_u32}>|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, _): (u32, DragPoint)| landed.update(|landed| landed.push(("outer", payload)))}
                >|_: DropHandle|
                    <Frame padding=20.0>
                        <DropTarget
                            accepts={Func::new(|payload: u32| payload == 1)}
                            on_drop={move |(payload, _): (u32, DragPoint)| landed.update(|landed| landed.push(("inner", payload)))}
                        >|_: DropHandle|
                            <DropTarget
                                accepts={Func::new(|_: String| true)}
                                on_drop={move |(_, _): (String, DragPoint)| landed.update(|landed| landed.push(("text", 0)))}
                            >|_: DropHandle|
                                <Frame width=60.0 height=60.0 />
                            </DropTarget>
                        </DropTarget>
                    </Frame>
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let middle = harness.center(target);
    harness.drag(harness.center(first), middle);
    harness.drag(harness.center(second), middle);

    assert_eq!(
        landed.get_untracked(),
        [("inner", 1), ("outer", 2)],
        "the inner target takes what it accepts and leaves the rest to the one around it"
    );
}
