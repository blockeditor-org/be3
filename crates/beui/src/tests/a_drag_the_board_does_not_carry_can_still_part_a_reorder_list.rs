use super::*;
use crate::geometry::Rect;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{
    DragPoint, Reorder, ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove,
};

#[test]
fn a_drag_the_board_does_not_carry_can_still_part_a_reorder_list() {
    let _root = RootScope::new();
    let keys = create_signal(vec![1_u32, 2, 3]);
    let moves = create_signal(Vec::<ReorderMove<u32, u32>>::new());
    let held = std::rc::Rc::new(std::cell::Cell::new(None::<ReorderHandle<u32, u32>>));
    let holding = std::rc::Rc::clone(&held);
    let document = build_node(move || {
        view! {
            <Reorder
                keys={keys}
                payload={Func::new(|key: u32| Some(key))}
                on_move={move |moved: ReorderMove<u32, u32>| moves.update(|moves| moves.push(moved))}
            >|reorder: ReorderHandle<u32, u32>| {
                holding.set(Some(reorder));
                view! {
                    <List spacing=0.0>
                        <ForEach keys={reorder.order}>|key: u32|
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
    let reorder = held.get().expect("the list hands out its handle");
    let over = DragPoint {
        item: Rect::from_min_size(pos2(-10.0, -10.0), vec2(100.0, REORDER_ROW)),
        ..DragPoint::at(
            harness.center(harness.find("row.3")) - vec2(0.0, 4.0),
            Modifiers::NONE,
        )
    };
    with_reactive_scope(&mut harness.document_mut(), || reorder.hover(Some(over)));
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.3"),
        3.0 * REORDER_ROW,
        "a drag the editor follows itself, such as the host's block drag, opens the same gap"
    );
    with_reactive_scope(&mut harness.document_mut(), || reorder.drop_at(9, over));
    let moved = moves.get_untracked();
    assert_eq!(moved.len(), 1);
    assert_eq!(
        (moved[0].key, moved[0].after, moved[0].index),
        (None, Some(2), 2)
    );
    harness.settle();
    assert_eq!(
        top_of(&harness, "row.3"),
        2.0 * REORDER_ROW,
        "and the gap closes on the drop"
    );
}
