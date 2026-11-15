use super::*;
use crate::primitives::*;
use crate::unstyled::{TextArea, TextAreaState};
use beui_text_editor_core::TextBuffer;
use std::sync::Arc;

const WIDTH: f32 = 300.0;

fn texts(document: &Document, id: NodeId, out: &mut Vec<NodeId>) {
    if document.arena.kind_of::<TextNode>(id).is_some() {
        out.push(id);
    }
    for child in document.children(id) {
        texts(document, child, out);
    }
}

fn shown_lines(harness: &Harness) -> Vec<String> {
    let document = harness.document();
    let mut found = Vec::new();
    texts(document, document.root().expect("a root"), &mut found);
    let mut lines = Vec::new();
    for id in found {
        let rect = document.node_rect(id).expect("the text was placed");
        assert!(
            rect.max.x <= WIDTH,
            "a row of the text area reaches {} past the area's {WIDTH}",
            rect.max.x
        );
        let node = kind_of::<TextNode>(document, id);
        let Some(layout) = document.text_layout(node) else {
            continue;
        };
        assert!(layout.size.x <= rect.width() + 0.5);
        for line in &layout.lines {
            lines.push(document.text(node)[line.range.clone()].to_owned());
        }
    }
    lines
}

#[test]
fn typing_a_long_word_into_a_text_area_never_widens_its_lines() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <Frame width={WIDTH} height=400.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    state={state}
                    scroll={plain_scroll()}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(100.0, 20.0));
    harness.frame(Vec::new());

    harness.type_text("abcd ");
    for _ in 0..8 {
        harness.type_text("eeeeeeeeee");
        harness.frame(Vec::new());
        let lines = shown_lines(&harness);
        if lines.iter().filter(|line| !line.is_empty()).count() > 1 {
            assert!(
                lines.iter().any(|line| line.starts_with("abcd eee")),
                "the e's start on the line after \"abcd \" rather than below it: {lines:?}"
            );
        }
    }
    for _ in 0..12 {
        harness.type_text(" abcd");
    }
    harness.frame(Vec::new());
    let lines = shown_lines(&harness);
    assert!(
        lines
            .iter()
            .filter(|line| line.contains("abcd abcd"))
            .count()
            > 1,
        "the short words wrap inside the area: {lines:?}"
    );
}
