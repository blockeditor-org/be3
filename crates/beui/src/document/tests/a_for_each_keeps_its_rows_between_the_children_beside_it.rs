use super::*;
use crate::reactive::{Button, Direction, ForEach, Frame, List, Text, build, create_signal, view};

#[test]
fn a_for_each_keeps_its_rows_between_the_children_beside_it() {
    let (lead, trail, add) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let (lead, trail, add) = (lead.clone(), trail.clone(), add.clone());
        move || {
            let (items, set_items) = create_signal(vec![1u32, 2]);
            view! {
                <List direction=Direction::Horizontal spacing=0.0>
                    <Frame @node_ref=&lead width=10.0 />
                    <ForEach keys={items}>
                        {|value: u32| view! {
                            <Frame width={value as f32 * 10.0} />
                        }}
                    </ForEach>
                    <Frame @node_ref=&trail width=5.0 />
                    <Button @node_ref=&add on_click={move || set_items.set(vec![1, 2, 3])}>
                        <Text string="add" />
                    </Button>
                </List>
            }
        }
    });

    let (lead, trail, add) = (lead.get(), trail.get(), add.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(harness.rect(lead).left(), 0.0);
    assert_eq!(
        harness.rect(trail).left(),
        40.0,
        "the rows of a `ForEach` are laid out by the row around it, after the child before it"
    );

    harness.click(harness.center(add));
    harness.frame(Vec::new());

    assert_eq!(
        harness.rect(trail).left(),
        70.0,
        "a row that arrives takes its place among the rows without moving past its neighbour"
    );
}
