use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_value_wrapped_in_a_list_item_takes_its_sizing() {
    let _root = crate::reactive::RootScope::new();
    let (fixed, filling) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let handed_over = view! {
            <Frame @node_ref=&filling />
        };
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <ListItem length=30.0>
                    <Frame @node_ref=&fixed />
                </ListItem>
                <ListItem grow=1.0>{handed_over}</ListItem>
            </List>
        }
    });

    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(fixed.get()).width(), 30.0);
    assert_eq!(
        harness.rect(filling.get()).width(),
        WIDE_VIEWPORT.x - 30.0,
        "a node handed over as a value takes the sizing of the `ListItem` around it"
    );
}
