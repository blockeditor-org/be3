use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn turning_off_animations_in_the_inspector_stops_a_scroll_at_its_end() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <ForEach keys={indices(40)}>|_: usize|
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Spacer />
                        </Frame>
                    </ForEach>
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(harness.document().motion().animates());

    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Simulation);
    harness.frame(Vec::new());
    harness.choose_in_simulation("motion", Some(beui_core::preferences::Motion::Instant));
    harness.frame(Vec::new());
    harness.toggle_inspector();
    harness.frame(Vec::new());
    assert!(!harness.document().motion().animates());

    let x = harness.rect(scroll).center().x;
    let top = harness.rect(scroll).top();
    harness.touch(TouchPhase::Start, pos2(x, top + 20.0));
    harness.touch(TouchPhase::Move, pos2(x, top + 220.0));

    assert_eq!(harness.document().scroll_overscroll(scroll), 0.0);
    assert_eq!(harness.document().scroll_offset(scroll), 0.0);

    harness.touch(TouchPhase::Move, pos2(x, top + 180.0));

    assert_eq!(
        harness.document().scroll_offset(scroll),
        40.0,
        "dragging back scrolls at once instead of first undoing the drag past the end"
    );
}
