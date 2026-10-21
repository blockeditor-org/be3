use super::*;
use crate::reactive::*;

#[component]
fn Deferred(children: Render) -> NodeId {
    view! {
        <styled::Frame>{children.call(())}</styled::Frame>
    }
}

#[test]
fn component_children_keep_their_owner_when_rendered_later() {
    let document = build_node(|| {
        view! {
            <Deferred>||
                <primitives::List spacing=0.0>
                    <styled::Frame>
                        <styled::Text>"Nested"</styled::Text>
                    </styled::Frame>
                </primitives::List>
            </Deferred>
        }
    });
    assert_eq!(
        document.component_tree().text(&document, false),
        concat!(
            "beui::tests::component_children_keep_their_owner_when_rendered_later::Deferred\n",
            "  primitives::List\n",
            "    styled::Frame\n",
            "      styled::Text\n",
            "        \"Nested\"\n",
        )
    );
}
