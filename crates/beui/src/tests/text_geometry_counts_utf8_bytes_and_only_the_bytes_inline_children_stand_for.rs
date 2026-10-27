use super::*;
use crate::icons::ICON_STAR;
use crate::primitives::{Gap, Icon, Span};
use crate::primitives::*;
use crate::reactive::*;
use crate::{Color32, FontId, TextGlyph};

fn laid_out(document: Document, text: NodeRef) -> TextGeometry {
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| {
        measure_text(text.get()).get_untracked()
    })
    .expect("a laid-out text is measured")
}

fn bytes(geometry: &TextGeometry) -> std::ops::Range<usize> {
    let lines = geometry.lines();
    lines[0].bytes.start..lines[lines.len() - 1].bytes.end
}

fn carets(geometry: &TextGeometry, end: usize) -> Vec<Option<Rect>> {
    (0..=end)
        .map(|index| geometry.caret_rect(index, 1.0))
        .collect()
}

#[test]
fn text_geometry_counts_utf8_bytes_and_only_the_bytes_inline_children_stand_for() {
    let _root = crate::reactive::RootScope::new();
    let font = FontId::proportional(14.0);

    let text = NodeRef::new();
    let wide = laid_out(
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>"a\u{e9}\u{20ac}b"</Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(
        bytes(&wide),
        0..7,
        "offsets are UTF-8 bytes: a, \u{e9} (2), \u{20ac} (3), b"
    );
    let after_e = wide.caret_rect(3, 1.0).expect("a byte index has a caret");
    let after_euro = wide.caret_rect(6, 1.0).expect("a byte index has a caret");
    assert!(after_e.min.x < after_euro.min.x);
    assert_eq!(
        wide.index_at(after_euro.center()),
        Some(6),
        "the index under a caret is its byte offset"
    );

    let text = NodeRef::new();
    let gapped = laid_out(
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>
                        "ab"
                        <Gap width=20.0 />
                        <Icon glyph={Some(ICON_STAR)} />
                        "cd"
                    </Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(bytes(&gapped), 0..4, "a Gap and an Icon hold no bytes");

    let text = NodeRef::new();
    let item = laid_out(
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>
                        "ab"
                        <TextItem>
                            <Frame width=10.0 height=10.0 />
                        </TextItem>
                        "cd"
                    </Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(
        bytes(&item),
        0..7,
        "an inline TextItem with no text stands for U+FFFC, three bytes"
    );

    let text = NodeRef::new();
    let described = laid_out(
        build_node(move || {
            let stands = Some(std::rc::Rc::<str>::from("- "));
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>
                        <TextItem text={stands}>
                            <Frame width=10.0 height=10.0 />
                        </TextItem>
                        "cd"
                    </Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(
        bytes(&described),
        0..4,
        "an inline TextItem given text stands for that text's bytes"
    );

    let text = NodeRef::new();
    let anchored = laid_out(
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>
                        "abcd"
                        <TextItem at={Some(1)}>
                            <Frame width=10.0 height=10.0 />
                        </TextItem>
                    </Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(
        bytes(&anchored),
        0..4,
        "a TextItem placed at an index holds no bytes"
    );

    let text = NodeRef::new();
    let plain = laid_out(
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE>
                        <Span>"a b\tc"</Span>
                    </Text>
                </List>
            }
        }),
        text,
    );
    let text = NodeRef::new();
    let marked = laid_out(
        build_node(move || {
            let glyphs = vec![
                TextGlyph::new(1..2, "\u{b7}", font, Color32::WHITE),
                TextGlyph::new(3..4, "\u{2192}", font, Color32::WHITE),
            ];
            view! {
                <List spacing=0.0>
                    <Text @node_ref=&text color=Color32::WHITE glyphs={glyphs}>
                        <Span>"a b\tc"</Span>
                    </Text>
                </List>
            }
        }),
        text,
    );
    assert_eq!(
        bytes(&marked),
        bytes(&plain),
        "a glyph counts as the bytes it is drawn over"
    );
    assert_eq!(
        carets(&marked, 5),
        carets(&plain, 5),
        "a glyph takes no room: every caret sits where it would without it"
    );
}
