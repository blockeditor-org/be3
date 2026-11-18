use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::{Reorder, Text};
use crate::unstyled::{ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove};

#[test]
fn a_fixed_lists_carried_row_is_read_once_as_its_preview() {
    let _root = RootScope::new();
    let document = build_node(move || {
        let keys = create_signal(vec![1_u32, 2, 3]);
        view! {
            <Frame width=REORDER_COLUMN>
                <Reorder
                    value={keys}
                    fixed=true
                    payload={Func::new(|key: u32| Some(key))}
                    text={|key: u32| format!("Row {key}")}
                    preview={|key: u32| view! {
                        <Frame width=REORDER_COLUMN height=REORDER_ROW>
                            <Text>{format!("Row {key}")}</Text>
                        </Frame>
                    }}
                    on_move={|_: ReorderMove<u32, u32>| {}}
                >|reorder: ReorderHandle<u32, u32>|
                    <List spacing=0.0>
                        <ForEach value={reorder.order} handle={reorder.rows}>|key: u32|
                            <ReorderItem reorder item=key>|_: ReorderItemHandle|
                                <Frame @test_id={format!("row.{key}")} height=REORDER_ROW>
                                    <Text>{format!("Row {key}")}</Text>
                                </Frame>
                            </ReorderItem>
                        </ForEach>
                    </List>
                </Reorder>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let read = |harness: &mut Harness| {
        harness.frame(Vec::new());
        harness
            .accessible()
            .iter()
            .filter(|node| node.label() == Some("Row 1") || node.value() == Some("Row 1"))
            .count()
    };
    assert_eq!(read(&mut harness), 1, "a row at rest is read once");

    let grab = harness.center(harness.find("row.1"));
    harness.down(grab, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 30.0))]);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 40.0))]);
    harness.settle();
    assert_eq!(
        read(&mut harness),
        1,
        "carried, it is read as its preview, not also as its empty slot"
    );
    harness.up(grab + vec2(0.0, 40.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(read(&mut harness), 1, "and once again when it lands");
}
