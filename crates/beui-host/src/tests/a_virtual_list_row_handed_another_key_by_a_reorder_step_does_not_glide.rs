use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::Reorder;
use crate::unstyled::{ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove, Scroll};

#[test]
fn a_virtual_list_row_handed_another_key_by_a_reorder_step_does_not_glide() {
    let _root = RootScope::new();
    let keys = create_signal((0..50_u32).collect::<Vec<_>>());
    let held = StoredCell::new(None::<ReorderHandle<u32, u32>>);
    let document = build_node(move || {
        view! {
            <Frame height={5.0 * REORDER_ROW}>
                <Scroll style={plain_scroll()}>
                    <Reorder
                        value={keys}
                        payload={Func::new(|key: u32| Some(key))}
                        preview={|_: u32| view! {
                            <Frame height=REORDER_ROW />
                        }}
                        on_move={move |moved: ReorderMove<u32, u32>| keys.update(|keys| {
                            keys.retain(|key| *key != moved.payload);
                            keys.insert(moved.index, moved.payload);
                        })}
                    >|reorder: ReorderHandle<u32, u32>| {
                        held.set(Some(reorder));
                        view! {
                            <VirtualList
                                value={reorder.order}
                                handle={reorder.rows}
                                item_size=REORDER_ROW
                                overscan=0.0
                            >|key: ReadSignal<u32>|
                                <ReorderItem reorder item={key}>|_: ReorderItemHandle|
                                    <Frame
                                        @test_id={move || format!("row.{}", key.get())}
                                        height=REORDER_ROW
                                    />
                                </ReorderItem>
                            </VirtualList>
                        }
                    }</Reorder>
                </Scroll>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let reorder = held.get().expect("the reorder handed its handle");

    assert!(
        reorder.step(0, 30),
        "a step moves the first row far below the view"
    );
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.1"),
        REORDER_ROW,
        "the rows the step moved up are painted where they were"
    );
    assert_eq!(
        top_of(&harness, "row.5"),
        4.0 * REORDER_ROW,
        "but the row handed a key it did not show before rests in its place at once"
    );
}
