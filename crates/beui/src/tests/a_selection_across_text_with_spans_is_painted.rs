use super::*;
use crate::reactive::{NodeRef, Span, Text, build, view};
use crate::styled::SelectableText;

#[test]
fn a_selection_across_text_with_spans_is_painted() {
    let text = NodeRef::new();
    let document = build({
        let text = text.clone();
        move || {
            view! {
                <SelectableText>
                    <Text @node_ref=&text>
                        <Span color=Color32::WHITE>"colored "</Span>
                        <Span font={crate::FontId::proportional(14.0).bold(true)}>"words"</Span>
                    </Text>
                </SelectableText>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(text.get());
    harness.drag(
        pos2(rect.left() + 1.0, rect.center().y),
        pos2(rect.right() - 1.0, rect.center().y),
    );
    let output = harness.frame(Vec::new());

    let highlighted = output.shapes().iter().any(|shape| {
        matches!(shape, crate::Shape::Rect { color, stroke_width, .. }
            if *color == styled::Theme::DARK.accent_soft && *stroke_width == 0.0)
    });
    assert!(
        highlighted,
        "the selection is painted behind text that has spans"
    );
}
