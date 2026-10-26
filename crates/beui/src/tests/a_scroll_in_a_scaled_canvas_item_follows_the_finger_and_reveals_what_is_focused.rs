use super::*;
use crate::input::TOUCH_DRAG_THRESHOLD;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

const SCALE: f32 = 2.0;

#[test]
fn a_scroll_in_a_scaled_canvas_item_follows_the_finger_and_reveals_what_is_focused() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let buttons: Vec<NodeRef> = (0..10).map(|_| NodeRef::new()).collect();
    let document = build_node({
        let buttons = buttons.clone();
        move || {
            view! {
                <Canvas view={Some(CanvasView::new(pos2(10.0, 10.0), SCALE))}>
                    <CanvasItem x=0.0 y=0.0 width=200.0 height=120.0 scales=true>
                        <Scroll @node_ref=&scroll>
                            <ForEach keys={indices(buttons.len())}>|index: usize|
                                <LabelledButton
                                    @node_ref={&buttons[index]}
                                    label={format!("Button {index}")}
                                />
                            </ForEach>
                        </Scroll>
                    </CanvasItem>
                </Canvas>
            }
        }
    });
    let scroll = scroll.get();
    let buttons: Vec<NodeId> = buttons.iter().map(NodeRef::get).collect();
    let mut harness = Harness::sized(document, vec2(800.0, 600.0));
    harness.frame(Vec::new());
    let shown = harness.rect(scroll);

    for _ in 0..8 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    let rect = harness.rect(buttons[7]);
    assert!(
        harness.document().scroll_offset(scroll) > 0.0 && (rect.bottom() - shown.bottom()).abs() < 0.5,
        "tabbing down to a button scrolls the zoomed list just far enough to show it, \
         not {rect:?} in {shown:?}"
    );
    for _ in 0..7 {
        harness.key(Key::Tab, Modifiers::SHIFT);
    }
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    let start = pos2(shown.center().x, shown.bottom() - 20.0);
    let end = pos2(start.x, start.y - 100.0);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);
    harness.touch(TouchPhase::End, end);
    assert!(
        (harness.document().scroll_offset(scroll) - (100.0 - TOUCH_DRAG_THRESHOLD) / SCALE).abs()
            < 0.01,
        "the zoomed list moves with the finger, in its own units"
    );
}
