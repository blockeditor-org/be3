use super::*;
use crate::input::TOUCH_DRAG_THRESHOLD;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn touch_dragging_a_scroll_moves_it_without_activating_a_row() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(100)}>|index: usize|
                        <LabelledButton
                            label={format!("Row {index}")}
                            on_click={move || {
                                clicks.update(|clicks| *clicks += 1);
                            }}
                        />
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
    assert_eq!(harness.document().focused_node(), None);
    harness.touch(TouchPhase::End, end);
    let released_offset = harness.document().scroll_offset(scroll);
    assert!((released_offset - (100.0 - TOUCH_DRAG_THRESHOLD)).abs() < 0.01);

    harness.frame(Vec::new());

    assert!(harness.document().scroll_offset(scroll) > released_offset);
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
    assert_eq!(harness.document().focused_node(), None);
    assert_eq!(clicks.get_untracked(), 0);
}
