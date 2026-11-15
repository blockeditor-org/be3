use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::Reorder;
use crate::unstyled::{ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove};

#[test]
fn a_reorder_step_glides_only_the_rows_it_moves_and_an_order_changed_otherwise_snaps() {
    let _root = RootScope::new();
    let keys = create_signal(vec![0_u32, 1, 2, 3]);
    let held = StoredCell::new(None::<ReorderHandle<u32, u32>>);
    let document = build_node(move || {
        view! {
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
                    <List spacing=0.0>
                        <ForEach value={reorder.order} handle={reorder.rows}>|key: u32|
                            <ReorderItem reorder item=key>|_: ReorderItemHandle|
                                <Frame @test_id={format!("row.{key}")} height=REORDER_ROW />
                            </ReorderItem>
                        </ForEach>
                    </List>
                }
            }</Reorder>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let reorder = held.get().expect("the reorder handed its handle");

    assert!(reorder.step(2, -1), "a step moves the row up one");
    harness.frame(Vec::new());
    assert_eq!(keys.get_untracked(), [0, 2, 1, 3]);
    assert_eq!(
        top_of(&harness, "row.2"),
        2.0 * REORDER_ROW,
        "the first frame after a step paints the rows it moved where they were"
    );
    assert_eq!(top_of(&harness, "row.1"), REORDER_ROW);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let rising = top_of(&harness, "row.2");
    assert!(
        rising > REORDER_ROW && rising < 2.0 * REORDER_ROW,
        "then glides them, at {rising}"
    );
    assert_eq!(top_of(&harness, "row.0"), 0.0, "and leaves the rows it did not move");
    assert_eq!(top_of(&harness, "row.3"), 3.0 * REORDER_ROW);
    harness.settle();

    keys.set(vec![3, 0, 2, 1]);
    harness.frame(Vec::new());
    assert_eq!(
        top_of(&harness, "row.3"),
        0.0,
        "an order changed by anything but the reorder snaps at once"
    );
    assert_eq!(top_of(&harness, "row.1"), 3.0 * REORDER_ROW);
}
