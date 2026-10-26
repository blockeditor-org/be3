use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::primitives::List;

const CONTENT_HEIGHT: f32 = 2000.0;
const EDGE: f32 = 2.0;

#[test]
fn clicking_the_scrollbar_track_pages_the_scroll_towards_the_click() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let scroll = scroll.get();
    let bar = harness.rect(harness.document().children(scrollbar_of(&harness, scroll))[0]);

    harness.click(pos2(bar.center().x, bar.bottom() - EDGE));
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(scroll), VIEWPORT.y);

    harness.click(pos2(bar.center().x, bar.bottom() - EDGE));
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(scroll), VIEWPORT.y * 2.0);

    harness.click(pos2(bar.center().x, bar.top() + EDGE));
    harness.frame(Vec::new());

    assert_eq!(harness.document().scroll_offset(scroll), VIEWPORT.y);
}
