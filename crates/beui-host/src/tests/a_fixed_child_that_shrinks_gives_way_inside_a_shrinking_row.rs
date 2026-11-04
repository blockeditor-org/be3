use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_fixed_child_that_shrinks_gives_way_inside_a_shrinking_row() {
    let _root = crate::reactive::RootScope::new();
    let (field, button) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <ListItem shrink=1.0>
                    <Frame>
                        <List direction=Direction::Horizontal spacing=10.0>
                            <ListItem length=300.0 shrink=1.0>
                                <Frame @node_ref=&field height=20.0 />
                            </ListItem>
                            <Frame @node_ref=&button width=60.0 height=20.0 />
                        </List>
                    </Frame>
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 50.0));
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(field.get()).width(),
        130.0,
        "the fixed child gives way so its row fits the frame that shrank"
    );
    assert_eq!(harness.rect(button.get()).right(), 200.0);

    harness.resize(vec2(500.0, 50.0));
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(field.get()).width(),
        300.0,
        "with room the fixed child keeps its length"
    );
}
