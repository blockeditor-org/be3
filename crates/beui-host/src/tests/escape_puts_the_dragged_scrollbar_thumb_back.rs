use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;

const CONTENT_HEIGHT: f32 = 2000.0;
const DRAGGED: f32 = 100.0;

#[test]
fn escape_puts_the_dragged_scrollbar_thumb_back() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Scroll @node_ref=&scroll>
                        <Frame height=CONTENT_HEIGHT />
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let scroll = scroll.get();
    let bar = harness.rect(harness.document().children(scrollbar_of(&harness, scroll))[0]);
    let thumb = bar.height() * VIEWPORT.y / CONTENT_HEIGHT;
    let grabbed = pos2(bar.center().x, bar.top() + thumb / 2.0);

    harness.down(grabbed, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(pos2(grabbed.x, grabbed.y + DRAGGED))]);
    harness.frame(Vec::new());
    assert!(
        harness.document().scroll_offset(scroll) > 0.0,
        "the thumb scrolls the content as it is dragged"
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![Input::PointerMoved(pos2(grabbed.x, grabbed.y + 2.0 * DRAGGED))]);
    harness.up(pos2(grabbed.x, grabbed.y + 2.0 * DRAGGED), PointerButton::Primary);
    harness.settle();

    assert_eq!(
        harness.document().scroll_offset(scroll),
        0.0,
        "Escape scrolls back to where the thumb was pressed, and the release leaves it there"
    );
}
