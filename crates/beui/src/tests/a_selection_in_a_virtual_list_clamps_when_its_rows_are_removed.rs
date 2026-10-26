use super::*;
use crate::primitives::{Text, VirtualList};
use crate::reactive::*;
use crate::styled::{Scroll, SelectableText};

fn label(index: usize) -> String {
    format!("Row {index}")
}

#[component]
fn Line(index: ReadSignal<usize>) -> NodeId {
    let text = create_memo(move || label(index.get()));
    let id = create_memo(move || format!("row.{}", index.get()));
    view! {
        <Text @test_id={id}>{text}</Text>
    }
}

#[test]
fn a_selection_in_a_virtual_list_clamps_when_its_rows_are_removed() {
    let _root = crate::reactive::RootScope::new();
    let region = NodeRef::new();
    let keys = create_signal((0..50).collect::<Vec<usize>>());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Fixed(100.0)>
                    <SelectableText @node_ref=&region>
                        <VirtualList keys item_size=20.0 text={label}>|index: ReadSignal<usize>|
                            <Line index />
                        </VirtualList>
                    </SelectableText>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let row = |harness: &Harness, index: usize| harness.rect(harness.find(&format!("row.{index}")));
    let selected = |harness: &mut Harness| selected_text(harness, region.get());
    let (first, last) = (row(&harness, 1), row(&harness, 3));
    harness.drag(
        pos2(first.left() + 1.0, first.center().y),
        pos2(last.right() + 20.0, last.center().y),
    );
    assert_eq!(selected(&mut harness), "Row 1\nRow 2\nRow 3");

    keys.set((2..50).collect());
    harness.frame(Vec::new());
    assert_eq!(
        selected(&mut harness),
        "Row 2\nRow 3",
        "a removed start row moves the start to the first row that remains"
    );

    keys.set([2].into_iter().chain(4..50).collect());
    harness.frame(Vec::new());
    assert_eq!(
        selected(&mut harness),
        "Row 2",
        "a removed end row moves the end to the row before it"
    );

    keys.set((10..50).collect());
    harness.frame(Vec::new());
    assert_eq!(
        selected(&mut harness),
        "",
        "removing every selected row leaves nothing selected"
    );
}
