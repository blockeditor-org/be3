use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Reorder, ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove};

#[test]
fn a_virtual_list_reorders_its_rows_by_key() {
    let _root = RootScope::new();
    let keys = create_signal((0..50_u32).collect::<Vec<_>>());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Percent(100.0)>
                    <Reorder
                        keys={keys}
                        payload={Func::new(|key: u32| Some(key))}
                        on_move={move |moved: ReorderMove<u32, u32>| keys.update(|keys| {
                            keys.retain(|key| *key != moved.payload);
                            keys.insert(moved.index, moved.payload);
                        })}
                    >|reorder: ReorderHandle<u32, u32>|
                        <VirtualList
                            keys={reorder.order}
                            item_size=REORDER_ROW
                        >|key: ReadSignal<u32>|
                            <ReorderItem reorder item={key}>|_: ReorderItemHandle|
                                <Frame
                                    @test_id={format!("row.{}", key.get_untracked())}
                                    height=REORDER_ROW
                                />
                            </ReorderItem>
                        </VirtualList>
                    </Reorder>
                </Offset>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("row.0"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 6.0))]);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 45.0))]);
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.0"),
        45.0,
        "the lifted row follows the pointer"
    );
    assert_eq!(
        top_of(&harness, "row.1"),
        0.0,
        "the rows it passed moved up"
    );
    assert_eq!(top_of(&harness, "row.2"), REORDER_ROW);
    harness.release_at(grab + vec2(0.0, 45.0));
    assert_eq!(keys.get_untracked()[..4], [1, 2, 0, 3]);
    harness.settle();
    assert_eq!(top_of(&harness, "row.0"), 2.0 * REORDER_ROW);
}
