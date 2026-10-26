use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use beui_core::rich::{CaretHandle, handle_center};
use crate::primitives::List;

#[test]
fn a_touch_selection_grows_by_its_handle_and_a_tap_outside_clears_it() {
    let _root = crate::reactive::RootScope::new();
    let (first, second, region) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <SelectableText @node_ref=&region>
                <List spacing=30.0>
                    <Text @node_ref=&first>"hello world"</Text>
                    <Text @node_ref=&second>"second line"</Text>
                </List>
            </SelectableText>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (first, second, region) = (
        kind_of::<TextNode>(harness.document(), first.get()),
        second.get(),
        region.get(),
    );
    let (top, bottom) = (harness.rect(first), harness.rect(second));
    let on_hello = pos2(top.left() + 4.0, top.center().y);

    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    assert_eq!(
        selected_text(&mut harness, region),
        "hello",
        "a double tap selects the word"
    );

    harness.advance(Duration::from_secs(1));
    let on_world = pos2(top.right() - 4.0, top.center().y);
    harness.touch(TouchPhase::Start, on_world);
    harness.touch(TouchPhase::End, on_world);
    assert_eq!(
        selected_text(&mut harness, region),
        "",
        "a tap away from the selection clears it"
    );

    harness.advance(Duration::from_secs(1));
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);
    harness.touch(TouchPhase::Start, on_hello);
    harness.touch(TouchPhase::End, on_hello);

    let caret = harness
        .document()
        .text_caret_rect(first, "hello".len(), 2.0)
        .expect("the selection's end has a place");
    let grip = handle_center(caret, CaretHandle::End);
    let grip = pos2(grip.x, grip.y);
    let target = pos2(bottom.right() + 20.0, bottom.center().y) + (grip - caret.min);
    harness.touch(TouchPhase::Start, grip);
    harness.touch(TouchPhase::Move, pos2(grip.x, grip.y + 20.0));
    harness.touch(TouchPhase::Move, target);
    harness.touch(TouchPhase::End, target);
    assert_eq!(
        selected_text(&mut harness, region),
        "hello world\nsecond line",
        "dragging the end handle carries the selection into the next text"
    );

    let inside = pos2(top.center().x, top.center().y);
    harness.touch(TouchPhase::Start, inside);
    harness.touch(TouchPhase::End, inside);
    assert!(
        harness
            .document()
            .api::<unstyled::ContextMenuApi>(region)
            .toolbar(harness.document())
            .is_some(),
        "a tap on the selection shows its toolbar"
    );
}
