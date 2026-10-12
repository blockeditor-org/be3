use super::*;
use crate::input::TOUCH_DRAG_THRESHOLD;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_finger_drag_from_a_capturing_button_scrolls_the_list_around_it() {
    let clicks = Rc::new(Cell::new(0));
    let cancels = Rc::new(Cell::new(0));
    let scroll = NodeRef::new();
    let document = build({
        let (clicks, cancels, scroll) = (clicks.clone(), cancels.clone(), scroll.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <ForEach keys={indices(100)}>
                            {move |_: usize| {
                                let (clicks, cancels) = (clicks.clone(), cancels.clone());
                                view! {
                                    <Interactive
                                        capture_presses=true
                                        on_click={move || clicks.set(clicks.get() + 1)}
                                        on_cancel={move || cancels.set(cancels.get() + 1)}
                                    >
                                        <Frame height=30.0 />
                                    </Interactive>
                                }
                            }}
                        </ForEach>
                    </Scroll>
                </List>
            }
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
    assert_eq!(cancels.get(), 1, "the row that took the press lets it go");
    assert_eq!(clicks.get(), 0);
}
