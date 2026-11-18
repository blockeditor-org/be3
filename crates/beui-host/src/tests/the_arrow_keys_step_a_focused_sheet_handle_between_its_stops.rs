use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_arrow_keys_step_a_focused_sheet_handle_between_its_stops() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let closed = create_signal(0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <Sheet
                    @node_ref=&sheet
                    extent=600.0
                    on_close={move || closed.update(|closed| *closed += 1)}
                >
                    <Frame height=1000.0 />
                </Sheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let height = |harness: &Harness| {
        harness
            .document()
            .node_rect(sheet.get())
            .map(|rect| rect.height())
    };
    let handle = harness
        .document()
        .find_test_id("sheet.handle")
        .expect("the handle");
    harness.click(harness.center(handle));
    harness.settle();
    assert_eq!(height(&harness), Some(300.0), "a click leaves it where it is");

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.settle();
    assert_eq!(height(&harness), Some(540.0), "Up takes it to the stop above");
    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.settle();
    assert_eq!(height(&harness), Some(540.0), "and no further than the top one");

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.settle();
    assert_eq!(height(&harness), Some(300.0), "Down takes it to the stop below");
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.settle();
    assert_eq!(
        (height(&harness), closed.get_untracked()),
        (Some(300.0), 0),
        "the lowest stop is as far as a key takes it: it never closes"
    );
}
