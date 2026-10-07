use super::*;
use crate::Motion;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

const CONTENT_HEIGHT: f32 = 2000.0;
const DRAGGED: f32 = 100.0;

#[test]
fn dragging_the_scrollbar_thumb_scrolls_once_released_with_gesture_motion_off() {
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
    harness.context().set_motion(Motion::Still);
    harness.frame(Vec::new());

    let scroll = scroll.get();
    let bar = harness.rect(harness.document().children(scroll)[1]);
    let thumb = bar.height() * VIEWPORT.y / CONTENT_HEIGHT;
    let grabbed = pos2(bar.center().x, bar.top() + thumb / 2.0);
    let to = pos2(grabbed.x, grabbed.y + DRAGGED);

    harness.frame(vec![Event::PointerMoved(grabbed)]);
    harness.frame(vec![Event::PointerButton {
        pos: grabbed,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(vec![Event::PointerMoved(to)]);

    assert_eq!(
        harness.document().scroll_offset(scroll),
        0.0,
        "the content holds still while the thumb is held"
    );

    harness.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    let wanted = DRAGGED / (bar.height() - thumb) * (CONTENT_HEIGHT - VIEWPORT.y);
    let offset = harness.document().scroll_offset(scroll);
    assert!(
        (offset - wanted).abs() <= 1.0,
        "the release scrolled to {offset} rather than {wanted}"
    );
}
