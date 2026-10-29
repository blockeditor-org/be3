use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Overflow, WrapAt};

const LONG: &str = "Whatever does not fit on the lines a text may take is left out, and the last line it shows says so";

#[test]
fn wrapped_text_ends_its_last_allowed_line_in_an_ellipsis() {
    let _root = crate::reactive::RootScope::new();
    let height = |max_lines: Option<u32>| {
        let text = NodeRef::new();
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame width=160.0>
                        <Text
                            @node_ref=&text
                            overflow={Overflow::Wrap { at: WrapAt::Words, max_lines }}
                        >
                            {LONG}
                        </Text>
                    </Frame>
                </List>
            }
        });
        let mut harness = Harness::sized(document, vec2(300.0, 400.0));
        harness.frame(Vec::new());
        let node = kind_of::<TextNode>(harness.document(), text.get());
        let layout = harness
            .document()
            .text_layout(node)
            .expect("the text was laid out");
        let rect = harness
            .document()
            .node_rect(text.get())
            .expect("the text was placed");
        (layout, rect)
    };

    let (whole, whole_rect) = height(None);
    assert!(
        whole.lines.len() > 2,
        "the text wraps onto more than two lines"
    );
    let (clamped, rect) = height(Some(2));
    assert_eq!(clamped.lines.len(), 2, "the text keeps to two lines");
    assert!(
        rect.height() < whole_rect.height(),
        "it measures as tall as the lines it shows"
    );
    assert!(rect.width() <= 160.0);
    let last = clamped.lines.last().expect("a line shows");
    let shown = last
        .runs
        .last()
        .and_then(|run| run.galley.as_ref())
        .expect("the last line ends in text");
    assert!(
        shown.text().ends_with('\u{2026}'),
        "the last line ends in an ellipsis, not {:?}",
        shown.text()
    );
    assert!(last.width <= 160.0, "the ellipsis fits the width");
    assert_eq!(
        last.range.end,
        LONG.len(),
        "the last line stands for the rest of the text"
    );
}
