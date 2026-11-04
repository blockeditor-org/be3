use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

const FIXED_WIDTH: f32 = 30.0;

#[test]
fn a_list_items_reactive_props_move_a_child_between_fixed_and_growing() {
    let _root = crate::reactive::RootScope::new();
    let (toggle, left, right) = (NodeRef::new(), NodeRef::new(), NodeRef::new());

    let document = build_node(move || {
        let shared = create_signal(false);
        let grow = create_memo(move || match shared.get() {
            true => 1.0,
            false => 0.0,
        });
        let length = create_memo(move || (!shared.get()).then_some(FIXED_WIDTH));
        view! {
            <List spacing=0.0>
                <Button
                    @node_ref=&toggle
                    on_click={move || shared.update(|shared| *shared = !*shared)}
                >
                    <Text>"toggle"</Text>
                </Button>
                <ListItem grow=1.0>
                    <List direction=Direction::Horizontal spacing=0.0>
                        <ListItem grow length>
                            <List @node_ref=&left spacing=0.0></List>
                        </ListItem>
                        <ListItem grow=1.0>
                            <List @node_ref=&right spacing=0.0></List>
                        </ListItem>
                    </List>
                </ListItem>
            </List>
        }
    });

    let (toggle, left, right) = (toggle.get(), left.get(), right.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(left).width(), FIXED_WIDTH);
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x - FIXED_WIDTH);

    harness.click(harness.center(toggle));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(left).width(),
        WIDE_VIEWPORT.x / 2.0,
        "memos behind a `ListItem`'s props must relay a new sizing to the row that lays the child out"
    );
    assert_eq!(harness.rect(right).width(), WIDE_VIEWPORT.x / 2.0);
}
