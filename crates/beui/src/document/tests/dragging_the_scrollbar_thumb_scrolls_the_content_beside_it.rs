use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

const CONTENT_HEIGHT: f32 = 2000.0;
const DRAGGED: f32 = 100.0;

#[test]
fn dragging_the_scrollbar_thumb_scrolls_the_content_beside_it() {
    let scroll = NodeRef::new();
    let held = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&held @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let scroll = scroll.get();
    let bar = harness.rect(harness.document().children(scroll)[1]);
    let thumb = bar.height() * VIEWPORT.y / CONTENT_HEIGHT;
    let grabbed = pos2(bar.center().x, bar.top() + thumb / 2.0);

    harness.drag(grabbed, pos2(grabbed.x, grabbed.y + DRAGGED));
    harness.frame(Vec::new());

    let max_offset = CONTENT_HEIGHT - VIEWPORT.y;
    let travel = bar.height() - thumb;
    let wanted = DRAGGED / travel * max_offset;
    let offset = harness.document().scroll_offset(scroll);
    assert!(
        (offset - wanted).abs() <= 1.0,
        "the drag scrolled to {offset} rather than {wanted}"
    );

    let moved = pos2(grabbed.x, grabbed.y + offset / max_offset * travel);
    harness.drag(moved, pos2(moved.x, bar.top() - travel));
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(scroll), 0.0);
}
