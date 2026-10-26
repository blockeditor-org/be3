use super::*;
use crate::primitives::{Break, Text};
use crate::reactive::*;
use crate::styled::SelectableText;

#[test]
fn dragging_across_a_break_in_a_text_copies_one_line() {
    let _root = crate::reactive::RootScope::new();
    let (region, text) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <Text @node_ref=&text>
                    "wrapped"
                    <Break />
                    "line"
                </Text>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(text.get());
    harness.drag(
        pos2(rect.left() + 1.0, rect.top() + 4.0),
        pos2(rect.right() + 20.0, rect.bottom() - 4.0),
    );
    assert!(
        rect.height() > 25.0,
        "the break puts the text on two lines: {rect:?}"
    );
    assert_eq!(
        selected_text(&mut harness, region.get()),
        "wrappedline",
        "a break holds no bytes, so a copy across it has no line break"
    );
}
