use super::*;
use crate::icons::{ICON_FOLDER, ICON_SAVE};
use crate::primitives::{Icon, Text};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn dragging_across_an_icon_and_its_label_copies_only_the_label() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <SelectableText>
                <Text @node_ref=&text>
                    <Icon glyph=ICON_SAVE spacing_em=0.25 />
                    "Save"
                    <Icon glyph=ICON_FOLDER spacing_em=0.25 />
                </Text>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(text.get());
    harness.drag(
        pos2(rect.left() + 1.0, rect.center().y),
        pos2(rect.right() + 20.0, rect.center().y),
    );
    let copied = harness
        .frame(pressed_with(Key::C, Modifiers::CTRL))
        .copied_text;
    assert_eq!(
        copied.as_deref(),
        Some("Save"),
        "an icon holds no bytes, so a copy carries only the label"
    );

    let node = harness
        .document()
        .arena
        .kind_of::<TextNode>(text.get())
        .expect("the node is a text");
    assert_eq!(harness.document().text(node), "Save");
    assert_eq!(
        harness.document().text_icons(text.get()),
        [ICON_SAVE, ICON_FOLDER]
    );
    let layout = harness
        .document()
        .text_layout(node)
        .expect("the text was laid out");
    let highlight = layout.selection_rects(0..4);
    assert_eq!(highlight.len(), 1);
    assert!(
        highlight[0].left() > 0.0 && highlight[0].right() < layout.size.x,
        "the highlight covers the label and neither icon beside it: {highlight:?} in {}",
        layout.size.x
    );
}
