use super::*;
use crate::reactive::Func;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn the_innermost_drop_target_that_accepts_the_payload_takes_the_drop() {
    let landed = Rc::new(RefCell::new(Vec::<(&str, u32)>::new()));
    let outer = Rc::clone(&landed);
    let inner = Rc::clone(&landed);
    let refused = Rc::clone(&landed);
    let (document, [first, second, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable payload={1_u32}>
                    {|_: DragHandle| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                </Draggable>
            },
            view! {
                <Draggable payload={2_u32}>
                    {|_: DragHandle| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                </Draggable>
            },
            view! {
                <DropTarget
                    on_drop={move |(payload, _): (u32, DragPoint)| outer.borrow_mut().push(("outer", payload))}
                >
                    {move |_: DropHandle| view! {
                        <Frame padding_horizontal=20.0 padding_vertical=20.0>
                            <DropTarget
                                accepts={Func::new(|payload: u32| payload == 1)}
                                on_drop={move |(payload, _): (u32, DragPoint)| inner.borrow_mut().push(("inner", payload))}
                            >
                                {move |_: DropHandle| view! {
                                    <DropTarget
                                        accepts={Func::new(|_: String| true)}
                                        on_drop={move |(_, _): (String, DragPoint)| refused.borrow_mut().push(("text", 0))}
                                    >
                                        {|_: DropHandle| view! {
                                            <Frame width=60.0 height=60.0 />
                                        }}
                                    </DropTarget>
                                }}
                            </DropTarget>
                        </Frame>
                    }}
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
        *landed.borrow(),
        [("inner", 1), ("outer", 2)],
        "the inner target takes what it accepts and leaves the rest to the one around it"
    );
}
