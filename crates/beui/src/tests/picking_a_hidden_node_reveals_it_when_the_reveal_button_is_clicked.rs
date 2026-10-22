use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn picking_a_hidden_node_reveals_it_when_the_reveal_button_is_clicked() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame padding=4.0>
                    <Frame padding=4.0>
                        <Frame padding=4.0>
                            <Frame padding=4.0>
                                <Text @node_ref=&text color=Color32::WHITE>"Hello"</Text>
                            </Frame>
                        </Frame>
                    </Frame>
                </Frame>
            </List>
        }
    });
    let text = text.get();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(
        harness.tree(),
        ["list", "  frame", "    frame", "      frame"]
    );

    harness.toggle_picking();
    assert!(harness.inspector().state().picking.get());

    let target = harness
        .document()
        .node_rect(text)
        .expect("the text was not laid out")
        .center();
    harness.click(target);
    harness.frame(Vec::new());

    assert!(!harness.inspector().state().picking.get());
    assert_eq!(harness.inspector().state().selected.get(), Some(text));
    assert_eq!(
        harness.tree(),
        ["list", "  frame", "    frame", "      frame"],
        "picking a node leaves the tree as it was"
    );
    assert!(harness.reveal_shown(), "a hidden pick offers to reveal it");

    harness.click(harness.reveal_center());
    harness.frame(Vec::new());

    assert_eq!(
        harness.tree(),
        [
            "list",
            "  frame",
            "    frame",
            "      frame",
            "        frame",
            "          text",
        ]
    );
}
