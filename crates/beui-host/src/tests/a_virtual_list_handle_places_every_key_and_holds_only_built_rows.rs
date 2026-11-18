use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn a_virtual_list_handle_places_every_key_and_holds_only_built_rows() {
    let _root = crate::reactive::RootScope::new();
    let keys = create_signal((0..VIRTUAL_ITEM_COUNT).collect::<Vec<usize>>());
    let rows = ForEachRef::<usize, NodeId>::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Scroll style={plain_scroll()}>
                        <VirtualList
                            value={keys}
                            handle={rows}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >|key: ReadSignal<usize>, index: ReadSignal<usize>|
                            <Frame
                                @test_id={format!("row.{}", key.get_untracked())}
                                height=VIRTUAL_ITEM_HEIGHT
                                fill={move || Color::from(match index.get() % 2 {
                                    0 => Color32::WHITE,
                                    _ => Color32::BLACK,
                                })}
                            />
                        </VirtualList>
                    </Scroll>
                </ListItem>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let last = VIRTUAL_ITEM_COUNT - 1;
    assert_eq!(
        rows.keys().len(),
        VIRTUAL_ITEM_COUNT,
        "every key has a place"
    );
    assert_eq!(rows.index_of(&last), Some(last));
    assert!(
        rows.nodes_of(&last).is_empty(),
        "a row not built holds no node"
    );
    let first = rows.nodes_of(&0);
    assert_eq!(
        first,
        vec![harness.find("row.0")],
        "a built row holds the node it built"
    );
    assert_eq!(rows.key_of(&first[0]), Some(0));
    let index = rows
        .index(&1)
        .expect("a built row has the place it was handed");
    assert_eq!(index.get_untracked(), 1);

    keys.update(|keys| keys.rotate_right(1));
    harness.settle();
    assert_eq!(rows.index_of(&last), Some(0));
    assert_eq!(
        index.get_untracked(),
        2,
        "the place a built row was handed follows its key"
    );
    assert_eq!(rows.nodes_of(&0), first, "and the row keeps its node");
}
