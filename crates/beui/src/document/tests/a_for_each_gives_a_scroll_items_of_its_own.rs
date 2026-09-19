use super::*;
use crate::reactive::{
    Button, ForEach, Frame, ItemSize, List, Scroll, Text, build, create_signal, view,
};

const ROW_HEIGHT: f32 = 40.0;

#[test]
fn a_for_each_gives_a_scroll_items_of_its_own() {
    let (scroll, add) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (scroll, add) = (scroll.clone(), add.clone());
        move || {
            let (items, set_items) = create_signal(vec![1u32, 2]);
            view! {
                <List spacing=0.0>
                    <Button @node_ref=&add on_click={move || set_items.set(vec![1, 2, 3])}>
                        <Text string="add" />
                    </Button>
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <ForEach keys={items}>
                            {|_value: u32| view! {
                                <Frame height=ROW_HEIGHT />
                            }}
                        </ForEach>
                    </Scroll>
                </List>
            }
        }
    });

    let (scroll, add) = (scroll.get(), add.get());
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().children(scroll).len(),
        2,
        "the rows of a `ForEach` inside a scroll are items of that scroll, not one item holding them"
    );

    harness.click(harness.center(add));
    harness.frame(Vec::new());

    assert_eq!(harness.document().children(scroll).len(), 3);
    let rows = harness.document().children(scroll);
    assert_eq!(
        harness.rect(rows[2]).top(),
        harness.rect(rows[0]).top() + ROW_HEIGHT * 2.0,
        "a row that arrives is placed after the rows already in the scroll"
    );
}
