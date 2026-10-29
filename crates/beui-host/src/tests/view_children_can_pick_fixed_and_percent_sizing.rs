use super::*;
use crate::native::with_document;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn view_children_can_pick_fixed_and_percent_sizing() {
    let _root = crate::reactive::RootScope::new();
    let left = create_signal(None);
    let right = create_signal(None);

    let document = build_node(move || {
        let tree = view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <List @sizing=ItemSize::Fixed(30.0) spacing=0.0></List>
                <List @sizing=ItemSize::Percent(100.0) spacing=0.0></List>
            </List>
        };
        let children = with_document(|document| document.children(tree));
        left.set(Some(children[0]));
        right.set(Some(children[1]));
        tree
    });

    let left = left.get_untracked().expect("fixed child was created");
    let right = right.get_untracked().expect("percent child was created");

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), 30.0);
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x - 30.0);
}
