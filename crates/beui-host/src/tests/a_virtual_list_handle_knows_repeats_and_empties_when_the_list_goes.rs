use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_virtual_list_handle_knows_repeats_and_empties_when_the_list_goes() {
    let _root = crate::reactive::RootScope::new();
    let shown = create_signal(true);
    let rows = ForEachRef::<usize, NodeId>::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Scroll style={plain_scroll()}>
                        <Show value={shown}>
                            <VirtualList
                                value={vec![0_usize, 1, 0]}
                                handle={rows}
                                item_size=VIRTUAL_ITEM_HEIGHT
                            >|_: ReadSignal<usize>, _: ReadSignal<usize>|
                                <Frame height=VIRTUAL_ITEM_HEIGHT />
                            </VirtualList>
                        </Show>
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    assert_eq!(
        rows.index_of(&0),
        Some(0),
        "a repeated key is known by its first place"
    );
    assert_eq!(rows.nodes_of(&0).len(), 2, "and holds every repeat's row");
    let mut built = Vec::new();
    rows.for_each_row(|row| built.push((row.key, row.index.get_untracked())));
    assert_eq!(
        built,
        [(0, 0), (1, 1), (0, 2)],
        "each repeat is handed its own place"
    );

    shown.set(false);
    harness.settle();
    assert!(rows.keys().is_empty(), "a list gone shows nothing");
    assert!(rows.nodes_of(&0).is_empty());
}
