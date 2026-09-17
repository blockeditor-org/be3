use super::*;
use crate::reactive::{Child, Column, Prop, Text, build, component, create_memo, view};

#[component]
fn Joined(prefix: Prop<String>, suffix: Prop<String>, children: Child) -> NodeId {
    let joined = create_memo(move || format!("{}-{}", prefix.get(), suffix.get()));
    view! {
        <Column spacing=0.0>
            <Text string=joined />
            {children}
        </Column>
    }
}

#[test]
fn required_props_can_be_written_in_any_order_and_as_children() {
    let declared = NodeRef::new();
    let reversed = NodeRef::new();
    let document = build({
        let declared = declared.clone();
        let reversed = reversed.clone();
        move || {
            view! {
                <Column spacing=0.0>
                    <Joined prefix="left" suffix="right">
                        <Text @node_ref=&declared string="declared" />
                    </Joined>
                    <Joined suffix="right" prefix="left">
                        <Text @node_ref=&reversed string="reversed" />
                    </Joined>
                </Column>
            }
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
            "column", "  column", "    text", "    text", "  column", "    text", "    text",
        ]
    );
}
