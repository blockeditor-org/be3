use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn view_children_can_pick_fixed_and_growing_sizing() {
    let _root = crate::reactive::RootScope::new();
    let left = create_signal(None);
    let right = create_signal(None);

    let document = build_node(move || {
        let tree = view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <ListItem length=30.0>
                    <List spacing=0.0></List>
                </ListItem>
                <ListItem grow=1.0>
                    <List spacing=0.0></List>
                </ListItem>
            </List>
        };
        let children = with_document(|document| document.children(tree));
        left.set(Some(children[0]));
        right.set(Some(children[1]));
        tree
    });

    let left = left.get_untracked().expect("fixed child was created");
    let right = right.get_untracked().expect("growing child was created");

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), 30.0);
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x - 30.0);
}
