use super::*;
use crate::reactive::{Direction, Frame, ItemSize, List, NodeRef, Text, view};

const LONG: &str = "a caption far too long to sit on one line of this narrow wrapping row";

#[test]
fn wrapped_rows_wrap_long_text_and_grow_percent_children() {
    let (caption, grown, fixed) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (caption, grown, fixed) = (caption.clone(), grown.clone(), fixed.clone());
        move || {
            view! {
                <List direction=Direction::Horizontal wrap=true spacing=10.0>
                    <Text @node_ref=&caption string=LONG wrap=true />
                    <Frame
                        @sizing=ItemSize::Percent(100.0)
                        @node_ref=&grown
                        width=40.0
                        height=20.0
                    />
                    <Frame @node_ref=&fixed width=40.0 height=20.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, vec2(150.0, 300.0));
    harness.frame(Vec::new());
    let caption = harness.rect(caption.get());
    assert!(
        caption.width() <= 150.0,
        "the caption wraps to the line, got {caption:?}"
    );
    assert!(
        caption.height() > 30.0,
        "the caption takes several lines, got {caption:?}"
    );
    let grown = harness.rect(grown.get());
    let fixed = harness.rect(fixed.get());
    assert_eq!(
        grown.top(),
        fixed.top(),
        "both frames share the second line"
    );
    assert_eq!(
        grown.width(),
        100.0,
        "the percent child takes the line's leftover"
    );
    assert_eq!(fixed.left(), 110.0);
}
