use super::*;
use crate::reactive::{
    ClickCatcher, Column, Frame, ItemSize, NodeRef, Scroll, Spacer, build, intrinsic, view,
};

#[test]
fn touch_overscroll_bands_without_hovering_a_row() {
    let hovered = Rc::new(Cell::new(false));
    let hover_sink = hovered.clone();
    let scroll = NodeRef::new();
    let scroll_ref = scroll.clone();
    let document = build(move || {
        let mut items = vec![intrinsic(view! {
            <ClickCatcher on_hover_change={move |value| hover_sink.set(value)}>
                <Frame padding_horizontal=0.0 padding_vertical=20.0><Spacer /></Frame>
            </ClickCatcher>
        })];
        items.extend((0..20).map(|_| {
            intrinsic(view! {
                <Frame padding_horizontal=0.0 padding_vertical=20.0><Spacer /></Frame>
            })
        }));
        view! {
            <Column spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll_ref children={items} />
            </Column>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let start = pos2(harness.rect(scroll).center().x, 20.0);
    let end = pos2(start.x, start.y + 100.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);

    let overscroll = harness.document().scroll_overscroll(scroll);
    assert!(overscroll < 0.0);
    assert!(overscroll.abs() < 100.0);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
    assert!(!hovered.get());
    assert_eq!(harness.document().focused_node(), None);

    harness.touch(TouchPhase::End, end);
    assert!(harness.document().scroll_is_animating(scroll));
    assert!(!hovered.get());
}
