use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::styled::{Reorder, Text};
use crate::unstyled::{ReorderHandle, ReorderItem, ReorderItemHandle, ReorderMove};

/// A row whose focus is within it stays in the accessibility tree while it is carried, so
/// a screen reader reading the focus keeps its place; the preview is read beside it.
#[test]
fn a_fixed_lists_carried_row_holding_the_focus_stays_read_in_its_slot() {
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
                                <Interactive focusable=true>
                                    <Frame @test_id={format!("row.{key}")} height=REORDER_ROW>
                                        <Text>{format!("Row {key}")}</Text>
                                    </Frame>
                                </Interactive>
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
    assert!(
        harness.document().focused_node().is_some(),
        "the press focused the row"
    );
    assert_eq!(
        read(&mut harness),
        2,
        "carried with the focus within it, its slot is still read, beside its preview"
    );
    harness.up(grab + vec2(0.0, 40.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(read(&mut harness), 1, "and once when it lands");
}
