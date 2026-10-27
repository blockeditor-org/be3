use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn percent_sized_children_still_size_an_intrinsic_lists_height() {
    let _root = crate::reactive::RootScope::new();
    let row = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <List @node_ref=&row direction=Direction::Horizontal spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(50.0) padding={Sides::symmetric(0.0, 20.0)}>
                        <Spacer />
                    </Frame>
                    <Frame @sizing=ItemSize::Percent(50.0) padding={Sides::symmetric(0.0, 20.0)}>
                        <Spacer />
                    </Frame>
                </List>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(row.get()).height(), 40.0);
}
