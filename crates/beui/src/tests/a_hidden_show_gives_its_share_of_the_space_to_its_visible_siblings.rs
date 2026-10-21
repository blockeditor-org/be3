use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_hidden_show_gives_its_share_of_the_space_to_its_visible_siblings() {
    let _root = crate::reactive::RootScope::new();
    let shown = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Show condition=false>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                </Show>
                <Show condition=true>
                    <Frame @sizing=ItemSize::Percent(100.0) @node_ref=&shown />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(vec![]);

    let rect = harness.rect(shown.get());
    assert_eq!(rect.top(), 0.0);
    assert_eq!(rect.height(), VIEWPORT.y);
}
