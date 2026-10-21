use super::*;
use crate::reactive::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[component]
fn Column(landed: Signal<Vec<&'static str>>, slot: Child) -> NodeId {
    view! {
        <DropTarget
            on_drop={move |(_, _): (u32, DragPoint)| landed.update(|landed| landed.push("column"))}
        >|_: DropHandle| {
            slot
        }</DropTarget>
    }
}

#[test]
fn a_drop_target_built_before_the_one_around_it_still_takes_the_drop() {
    let _root = crate::reactive::RootScope::new();
    let landed = create_signal(Vec::<&str>::new());
    let (document, [source, target]) = toolbar_of(move || {
        let slot = view! {
            <DropTarget
                on_drop={move |(_, _): (u32, DragPoint)| landed.update(|landed| landed.push("slot"))}
            >|_: DropHandle|
                <Frame width=60.0 height=60.0 />
            </DropTarget>
        };
        [
            view! {
                <Draggable payload={1_u32}>|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
            },
            view! {
                <Column landed slot />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.drag(harness.center(source), harness.center(target));

    assert_eq!(
        landed.get_untracked(),
        ["slot"],
        "the slot is inside the column, though it was built first and fills it exactly"
    );
}
