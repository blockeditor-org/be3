use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;
use beui::TOUCH_DRAG_THRESHOLD;

#[test]
fn a_finger_drag_from_a_capturing_button_scrolls_the_list_around_it() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let cancels = create_signal(0);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(100)}>|_: usize|
                        <Interactive
                            capture_presses=true
                            on_click={move || clicks.update(|clicks| *clicks += 1)}
                            on_cancel={move || cancels.update(|cancels| *cancels += 1)}
                        >
                            <Frame height=30.0 />
                        </Interactive>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let rect = harness.rect(scroll);
    let start = pos2(rect.center().x, rect.bottom() - 40.0);
    let end = pos2(start.x, start.y - 100.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);
    harness.touch(TouchPhase::End, end);

    assert!(
        (harness.document().scroll_offset(scroll) - (100.0 - TOUCH_DRAG_THRESHOLD)).abs() < 0.01,
        "the scroll around the row takes the drag"
    );
    assert_eq!(
        cancels.get_untracked(),
        1,
        "the row that took the press lets it go"
    );
    assert_eq!(clicks.get_untracked(), 0);
}
