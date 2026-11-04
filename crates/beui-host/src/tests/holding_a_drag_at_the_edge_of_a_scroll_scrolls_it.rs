use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::unstyled::{DragHandle, Draggable};

#[test]
fn holding_a_drag_at_the_edge_of_a_scroll_scrolls_it() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let source = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Draggable
                    payload={7_u32}
                    preview={|_: u32| view! {
                        <Frame width=40.0 height=40.0 />
                    }}
                    @node_ref=&source
                >|_: DragHandle|
                    <Frame width=40.0 height=40.0 />
                </Draggable>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(100)}>|index: usize|
                        <LabelledButton label={format!("Row {index}")} />
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let (scroll, source) = (scroll.get(), source.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let viewport = harness.rect(scroll);

    harness.press_at(harness.center(source));
    harness.frame(vec![Input::PointerMoved(viewport.center())]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().scroll_offset(scroll),
        0.0,
        "the middle of a scroll is not its edge"
    );

    let edge = pos2(viewport.center().x, viewport.bottom() - 4.0);
    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(Vec::new());
    let early = harness.document().scroll_offset(scroll);
    for _ in 0..10 {
        harness.frame(Vec::new());
    }
    let later = harness.document().scroll_offset(scroll);
    assert!(
        later > early && later > 0.0,
        "holding the pointer still at the edge keeps scrolling, from {early} to {later}"
    );

    harness.frame(vec![Input::PointerMoved(viewport.center())]);
    harness.frame(Vec::new());
    let left = harness.document().scroll_offset(scroll);
    for _ in 0..5 {
        harness.frame(Vec::new());
    }
    assert_eq!(
        harness.document().scroll_offset(scroll),
        left,
        "it stops once the pointer leaves the edge"
    );

    harness.frame(vec![Input::PointerMoved(edge)]);
    harness.frame(Vec::new());
    harness.release_at(edge);
    let dropped = harness.document().scroll_offset(scroll);
    for _ in 0..5 {
        harness.frame(Vec::new());
    }
    assert_eq!(
        harness.document().scroll_offset(scroll),
        dropped,
        "and once the drag ends"
    );
    assert!(
        harness
            .document()
            .overlay_rects(crate::WindowId::MAIN)
            .is_empty(),
        "an edge scroll shows no autoscroll marker"
    );
}
