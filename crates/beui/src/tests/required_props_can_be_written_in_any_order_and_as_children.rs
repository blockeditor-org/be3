use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[component]
fn Joined(prefix: Prop<String>, suffix: Prop<String>, children: Child) -> NodeId {
    let joined = create_memo(move || format!("{}-{}", prefix.get(), suffix.get()));
    view! {
        <List spacing=0.0>
            <Text>{joined}</Text>
            {children}
        </List>
    }
}

#[test]
fn required_props_can_be_written_in_any_order_and_as_children() {
    let _root = crate::reactive::RootScope::new();
    let declared = NodeRef::new();
    let reversed = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Joined prefix="left" suffix="right">
                    <Text @node_ref=&declared>"declared"</Text>
                </Joined>
                <Joined suffix="right" prefix="left">
                    <Text @node_ref=&reversed>"reversed"</Text>
                </Joined>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), declared.get()), "declared");
    assert_eq!(text_of(harness.document(), reversed.get()), "reversed");
    harness.toggle_inspector();
    assert_eq!(
        harness.tree(),
        [
            "list", "  list", "    text", "    text", "  list", "    text", "    text",
        ]
    );
}
