use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

const CONTENT_HEIGHT: f32 = 2000.0;

#[test]
fn the_scrollbar_thumb_brightens_under_the_pointer_and_while_it_is_dragged() {
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
    let theme = styled::Theme::DARK;
    let output = harness.frame(Vec::new());
    assert_eq!(painted(&output, theme.scroll_thumb), 1);

    let bar = harness.rect(harness.document().children(scroll.get())[1]);
    let thumb = bar.height() * VIEWPORT.y / CONTENT_HEIGHT;
    let over_thumb = pos2(bar.center().x, bar.top() + thumb / 2.0);
    let over_track = pos2(bar.center().x, bar.bottom() - 1.0);

    let output = harness.frame(vec![Event::PointerMoved(over_thumb)]);
    assert_eq!(painted(&output, theme.scroll_thumb_hover), 1);
    assert_eq!(painted(&output, theme.scroll_thumb), 0);

    let output = harness.frame(vec![Event::PointerMoved(over_track)]);
    assert_eq!(painted(&output, theme.scroll_thumb), 1);
    assert_eq!(painted(&output, theme.scroll_thumb_hover), 0);

    harness.frame(vec![Event::PointerMoved(over_thumb)]);
    let output = harness.frame(vec![Event::PointerButton {
        pos: over_thumb,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(painted(&output, theme.scroll_thumb_active), 1);

    let output = harness.frame(vec![Event::PointerMoved(pos2(over_thumb.x, bar.bottom()))]);
    assert_eq!(painted(&output, theme.scroll_thumb_active), 1);
}

fn painted(output: &crate::FrameOutput, color: Color32) -> usize {
    output
        .shapes()
        .iter()
        .filter(
            |shape| matches!(shape, crate::Shape::Rect { color: painted, .. } if *painted == color),
        )
        .count()
}
