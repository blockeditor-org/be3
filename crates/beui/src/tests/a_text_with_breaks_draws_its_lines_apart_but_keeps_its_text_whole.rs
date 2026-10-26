use super::*;
use crate::primitives::Span;
use crate::primitives::*;
use crate::reactive::*;
use crate::{Color32, FontId, Overflow, WrapAt};

#[test]
fn a_text_with_breaks_draws_its_lines_apart_but_keeps_its_text_whole() {
    let _root = crate::reactive::RootScope::new();
    let wrap = |at| Overflow::Wrap {
        at,
        max_lines: None,
    };
    for overflow in [
        Overflow::Visible,
        Overflow::Ellipsis,
        wrap(WrapAt::Words),
        wrap(WrapAt::Anywhere),
    ] {
        let text = NodeRef::new();
        let document = build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame width=400.0>
                        <Text
                            @node_ref=&text
                            color=Color32::WHITE
                            font=FontId::monospace(14.0)
                            overflow
                        >
                            "first"
                            <Break />
                            <Span bold=true>"second"</Span>
                            <Break />
                            "third"
                        </Text>
                    </Frame>
                </List>
            }
        });
        let mut harness = Harness::new(document);
        harness.frame(Vec::new());
        let text = kind_of::<TextNode>(harness.document(), text.get());

        assert_eq!(
            harness.document().text(text),
            "firstsecondthird",
            "breaks add nothing to the text a copy reads ({overflow:?})"
        );
        let layout = harness
            .document()
            .text_layout(text)
            .expect("the text was laid out");
        let ranges: Vec<_> = layout.lines.iter().map(|line| line.range.clone()).collect();
        assert_eq!(
            ranges,
            vec![0..5, 5..11, 11..16],
            "each break ends a line ({overflow:?})"
        );
        let geometry = harness.document().text_geometry(text);
        let second = geometry.caret_rect(5, 1.0).expect("an index has a caret");
        let start = geometry.caret_rect(0, 1.0).expect("an index has a caret");
        assert_eq!(
            second.min.x, start.min.x,
            "the index at a break starts the next line ({overflow:?})"
        );
        assert!(second.min.y > start.min.y);
    }
}
