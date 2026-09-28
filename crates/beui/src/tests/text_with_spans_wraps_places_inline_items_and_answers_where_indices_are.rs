use super::*;
use crate::reactive::{
    Frame, List, NodeRef, SpanKind, SpanStyle, Text, TextItem, TextSpan, build, view,
};
use crate::{Color32, FontId};

const WRAP: f32 = 90.0;
const BOX: f32 = 20.0;

#[test]
fn text_with_spans_wraps_places_inline_items_and_answers_where_indices_are() {
    let text = NodeRef::new();
    let item = NodeRef::new();
    let content = "hello \u{fffc} wide world of text";
    let body = SpanStyle::new(FontId::proportional(14.0), Color32::WHITE);
    let spans = vec![
        TextSpan::text(0..6, body),
        TextSpan {
            kind: SpanKind::Inline(0),
            ..TextSpan::text(6..9, body)
        },
        TextSpan::text(9..content.len(), SpanStyle { font: body.font.bold(true), ..body }),
    ];
    let document = build({
        let (text, item) = (text.clone(), item.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame width=WRAP>
                        <Text @node_ref=&text string={content.to_owned()} spans={spans} wrap=true>
                            <TextItem>
                                <Frame @node_ref=&item width=BOX height=BOX />
                            </TextItem>
                        </Text>
                    </Frame>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (text, item) = (text.get(), item.get());
    let geometry = harness.document().text_geometry(text);
    let placed = harness.rect(text);

    let layout = harness.document().text_layout(text).expect("the text was laid out");
    assert!(layout.lines.len() > 1, "the text wraps inside {WRAP} points");
    assert!(layout.size.x <= WRAP, "no line is wider than the wrap: {:?}", layout.size);
    for line in &layout.lines[1..] {
        let first = content.as_bytes()[line.range.start];
        assert_ne!(first, b' ', "lines break after spaces, not before them");
    }

    let hello = geometry.caret_rect(6, 1.0).expect("an index has a caret");
    let boxed = harness.rect(item);
    assert_eq!(boxed.min.x, hello.min.x, "the inline item sits where its span starts");
    assert_eq!(boxed.width(), BOX);

    let world = content.find("world").expect("the text has a world");
    let caret = geometry.caret_rect(world, 1.0).expect("an index has a caret");
    assert!(caret.min.y > placed.min.y, "a wrapped word is on a later line");
    let under = geometry
        .index_at(caret.center() + Vec2::new(0.5, 0.0))
        .expect("a point in the text has an index");
    assert_eq!(under, world, "the index under a caret is that caret's index");
}
