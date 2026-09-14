use super::*;
use crate::reactive::{Frame, NodeRef, Text, build, view};

#[test]
fn picking_a_node_reveals_it_in_the_tree() {
    let text = NodeRef::new();
    let document = build({
        let text = text.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Frame padding_horizontal=4.0 padding_vertical=4.0>
                        <Frame padding_horizontal=4.0 padding_vertical=4.0>
                            <Frame padding_horizontal=4.0 padding_vertical=4.0>
                                <Frame padding_horizontal=4.0 padding_vertical=4.0>
                                    <Text
                                        @node_ref=&text
                                        string="Hello"
                                        font_size=14.0
                                        color=Color32::WHITE
                                    />
                                </Frame>
                            </Frame>
                        </Frame>
                    </Frame>
                </Column>
            }
        }
    });
    let text = text.get();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(
        harness.tree(),
        ["column", "  frame", "    frame", "      frame"]
    );

    harness.toggle_picking();
    assert!(harness.inspector().state.picking.get());

    let target = harness
        .document
        .node_rect(text)
        .expect("the text was not laid out")
        .center();
    harness.click(target);
    harness.frame(Vec::new());

    assert!(!harness.inspector().state.picking.get());
    assert_eq!(harness.inspector().state.selected.get(), Some(text));
    assert_eq!(
        harness.tree(),
        [
            "column",
            "  frame",
            "    frame",
            "      frame",
            "        frame",
            "          text",
        ]
    );
}
