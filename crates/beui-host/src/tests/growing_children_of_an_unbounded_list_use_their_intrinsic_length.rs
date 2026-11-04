use super::*;
use crate::primitives::*;
use crate::reactive::*;

const ITEM_HEIGHT: f32 = 20.0;

#[test]
fn growing_children_of_an_unbounded_list_use_their_intrinsic_length() {
    let _root = crate::reactive::RootScope::new();
    let inner = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <List @node_ref=&inner spacing=0.0>
                    <ListItem grow=1.0>
                        <List direction=Direction::Horizontal spacing=0.0>
                            <Frame padding={Sides::symmetric(0.0, ITEM_HEIGHT / 2.0)}>
                                <Frame />
                            </Frame>
                        </List>
                    </ListItem>
                </List>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(inner.get()).height(), ITEM_HEIGHT);
}
