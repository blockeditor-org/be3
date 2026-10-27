use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn a_drop_target_that_hides_the_preview_lets_the_source_show_the_item_in_place() {
    let _root = crate::reactive::RootScope::new();
    let floating = create_signal(false);
    let (document, [source, plain, holding]) = toolbar_of(move || {
        [
            view! {
                <Draggable
                    payload={7_u32}
                    preview={|_: u32| view! {
                        <Frame @test_id="ghost" width=20.0 height=20.0 />
                    }}
                >|handle: DragHandle| {
                    create_effect(move || floating.set(handle.floating.get()));
                    view! {
                        <Frame width=40.0 height=40.0 />
                    }
                }</Draggable>
            },
            view! {
                <DropTarget on_drop={move |_: (u32, DragPoint)| {}}>|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
            view! {
                <DropTarget
                    hides_preview=true
                    on_drop={move |_: (u32, DragPoint)| {}}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.press_at(harness.center(source));
    harness.frame(vec![Input::PointerMoved(harness.center(plain))]);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("ghost").is_some(),
        "the preview floats over a plain target"
    );
    assert!(floating.get_untracked());

    harness.frame(vec![Input::PointerMoved(harness.center(holding))]);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("ghost").is_none(),
        "over a target that hides it, there is no floating preview"
    );
    assert!(
        !floating.get_untracked(),
        "and the source hears it is lifted in place"
    );
    assert!(harness.document().dragging(), "though the drag goes on");

    harness.frame(vec![Input::PointerMoved(harness.center(plain))]);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("ghost").is_some(),
        "it floats again once it leaves"
    );
    harness.release_at(harness.center(plain));
}
