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
                    <ListItem grow=1.0>
                        <Frame />
                    </ListItem>
                </Show>
                <Show condition=true>
                    <ListItem grow=1.0>
                        <Frame @node_ref=&shown />
                    </ListItem>
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
