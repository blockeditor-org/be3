use super::*;
use beui::styled::{Frame, List, Size, Space};
use beui_macros::view;
use std::cell::Cell;
use std::rc::Rc;

/// What used to fail to compile: a `Show` in a `Frame`, as a `ForEach` row, and inside a
/// `Dynamic` in a list. Where the slot needs a node built once (a `ListItem`, a row), the
/// control flow is held by a bare frame that sizes to its child.
#[test]
fn control_flow_that_builds_one_node_fills_any_one_child_slot() {
    let held = Rc::new(Cell::new(None));
    let given = held.clone();
    let document = build(move || {
        let shown = create_signal(false);
        given.set(Some(shown));
        let keys = vec![1u32, 2];
        view! {
            <Root>
                <List spacing=Space::Md>
                    <Frame @test_id="frame">
                        <Show value={shown}>
                            <Frame @test_id="in_frame" height=Size::Sm />
                        </Show>
                    </Frame>
                    <ListItem grow=1.0>
                        <Show
                            value={shown}
                            fallback={|| view! {
                                <Frame @test_id="item_fallback" height=Size::Sm />
                            }}
                        >
                            <Frame @test_id="in_item" height=Size::Lg />
                        </Show>
                    </ListItem>
                    <ForEach value={keys}>|key: u32|
                        <Show value={shown}>
                            <Frame @test_id={format!("row{key}")} height=Size::Sm />
                        </Show>
                    </ForEach>
                    <Dynamic value={shown}>|_: bool|
                        <Show value={shown}>
                            <Frame @test_id="in_dynamic" height=Size::Sm />
                        </Show>
                    </Dynamic>
                </List>
            </Root>
        }
    });
    let shown = held.get().expect("the view was built");
    let mut test = DocumentTest::new(document, Vec2::new(200.0, 400.0));
    assert!(!test.shown("in_frame") && test.shown("item_fallback"));
    assert_eq!(test.rect_of("frame").height(), 0.0);

    shown.set(true);
    test.frame(Vec::new());
    for id in ["in_frame", "in_item", "row1", "row2", "in_dynamic"] {
        assert!(test.shown(id), "{id} shows");
    }
    assert!(!test.shown("item_fallback"));
    assert_eq!(test.rect_of("frame"), test.rect_of("in_frame"));
}
