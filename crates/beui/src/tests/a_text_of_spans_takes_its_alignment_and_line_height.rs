use super::*;
use crate::reactive::{Frame, NodeRef, Span, Text, build, view};
use crate::{FontId, TextAlign};

const WIDTH: f32 = 300.0;
const LINE: f32 = 40.0;

#[test]
fn a_text_of_spans_takes_its_alignment_and_line_height() {
    let text = NodeRef::new();
    let document = build({
        let text = text.clone();
        move || {
            view! {
                <Frame width=WIDTH>
                    <Text @node_ref=&text align=TextAlign::Center line_height=LINE>
                        "centered "
                        <Span font={FontId::proportional(14.0).bold(true)}>"words"</Span>
                    </Text>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = kind_of::<TextNode>(harness.document(), text.get());
    let rect = harness.rect(text);
    let layout = harness
        .document()
        .text_layout(text)
        .expect("the text was laid out");
    let [line] = layout.lines.as_slice() else {
        panic!("the text is one line, laid out as {}", layout.lines.len());
    };

    assert_eq!(
        line.height, LINE,
        "the line is as tall as the text's line height"
    );
    let geometry = harness.document().text_geometry(text);
    let start = geometry.caret_rect(0, 0.0).expect("the start has a caret");
    let end = geometry
        .caret_rect(harness.document().text(text).len(), 0.0)
        .expect("the end has a caret");
    let left = start.min.x - rect.left();
    let right = rect.right() - end.min.x;
    assert!(left > 0.0, "the spans are pushed in from the left");
    assert!(
        (left - right).abs() <= 1.0,
        "the spans sit in the middle of the text: {left} from the left, {right} from the right"
    );
}
