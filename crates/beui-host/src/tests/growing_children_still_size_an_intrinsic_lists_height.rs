use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn growing_children_still_size_an_intrinsic_lists_height() {
    let _root = crate::reactive::RootScope::new();
    let row = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <List @node_ref=&row direction=Direction::Horizontal spacing=0.0>
                    <ListItem grow=0.5>
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Frame />
                        </Frame>
                    </ListItem>
                    <ListItem grow=0.5>
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Frame />
                        </Frame>
                    </ListItem>
                </List>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row.get()).height(), 40.0);
}
