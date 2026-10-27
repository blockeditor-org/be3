use super::*;
use crate::unstyled::{DragHandle, DragPoint, Draggable, DropHandle, DropTarget};

#[test]
fn a_drop_target_hears_the_size_of_the_item_dragged_over_it() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(None::<Rect>);
    let (document, [source, target]) = toolbar_of(move || {
        [
            view! {
                <Draggable payload={7_u32}>|_: DragHandle|
                    <Frame width=40.0 height=30.0 />
                </Draggable>
            },
            view! {
                <DropTarget
                    on_over={move |over: Option<(u32, DragPoint)>| {
                        if let Some((_, point)) = over {
                            heard.set(Some(point.item));
                        }
                    }}
                >|_: DropHandle|
                    <Frame width=100.0 height=100.0 />
                </DropTarget>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(source).height(), 30.0);

    let grabbed = harness.rect(source).min + Vec2::new(10.0, 5.0);
    harness.press_at(grabbed);
    harness.frame(vec![Input::PointerMoved(harness.center(target))]);

    assert_eq!(
        heard.get_untracked(),
        Some(Rect::from_min_size(
            pos2(-10.0, -5.0),
            harness.rect(source).size()
        )),
        "the target hears the carried item's rect, relative to where it was grabbed"
    );
    assert_eq!(
        harness.document().drag_board().item(),
        heard.get_untracked(),
        "and the board answers the same"
    );
}
