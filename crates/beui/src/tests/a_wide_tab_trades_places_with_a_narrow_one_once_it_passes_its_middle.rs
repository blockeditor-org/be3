use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{Reorder, ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove};

const WIDTHS: [f32; 3] = [80.0, 20.0, 40.0];

#[test]
fn a_wide_tab_trades_places_with_a_narrow_one_once_it_passes_its_middle() {
    let _root = RootScope::new();
    let keys = create_signal(vec![0_usize, 1, 2]);
    let document = build_node(move || {
        view! {
            <Reorder
                keys={keys}
                direction=Direction::Horizontal
                payload={Func::new(|key: usize| Some(key))}
                on_move={move |moved: ReorderMove<usize, usize>| keys.update(|keys| {
                    keys.retain(|key| *key != moved.payload);
                    keys.insert(moved.index, moved.payload);
                })}
            >|reorder: ReorderHandle<usize, usize>|
                <List direction=Direction::Horizontal spacing=4.0>
                    <ForEach keys={reorder.order}>|key: usize|
                        <ReorderItem reorder item=key>|_: ReorderItemHandle|
                            <Frame
                                @test_id={format!("tab.{key}")}
                                width={WIDTHS[key]}
                                height=24.0
                            />
                        </ReorderItem>
                    </ForEach>
                </List>
            </Reorder>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let left =
        |harness: &Harness, key: usize| harness.rect(harness.find(&format!("tab.{key}"))).left();
    let grab = harness.center(harness.find("tab.0"));
    harness.press_at(grab);
    harness.frame(vec![Input::PointerMoved(grab + vec2(6.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(grab + vec2(53.0, 0.0))]);
    harness.settle();
    assert_eq!(
        left(&harness, 1),
        84.0,
        "short of the narrow tab's middle (94), the wide tab's middle (93) leaves it alone"
    );
    harness.frame(vec![Input::PointerMoved(grab + vec2(55.0, 0.0))]);
    harness.settle();
    assert_eq!(
        left(&harness, 1),
        0.0,
        "past it, the narrow tab moves to the front"
    );
    harness.frame(vec![Input::PointerMoved(grab + vec2(40.0, 0.0))]);
    harness.settle();
    assert_eq!(
        left(&harness, 1),
        0.0,
        "and coming back a little does not swap them again: the narrow tab's middle is now at 10"
    );
    harness.release_at(grab + vec2(40.0, 0.0));
    assert_eq!(keys.get_untracked(), vec![1, 0, 2]);
}
