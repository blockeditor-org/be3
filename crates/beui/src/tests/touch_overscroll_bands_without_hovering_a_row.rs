use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Scroll;

#[test]
fn touch_overscroll_bands_without_hovering_a_row() {
    let _root = crate::reactive::RootScope::new();
    let hovered = create_signal(false);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                    <Interactive on_hover_change={hovered}>
                        <Frame padding={Sides::symmetric(0.0, 20.0)}>
                            <Spacer />
                        </Frame>
                    </Interactive>
                    <ForEach keys={indices(20)}>|_: usize|
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
    let start = pos2(harness.rect(scroll).center().x, 20.0);
    let end = pos2(start.x, start.y + 100.0);

    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::Move, end);

    let overscroll = harness.document().scroll_overscroll(scroll);
    assert!(overscroll < 0.0);
    assert!(overscroll.abs() < 100.0);
    assert_eq!(
        harness.document().scroll_offset(scroll),
        overscroll,
        "the content is pulled past its start by the overscroll"
    );
    assert!(!hovered.get_untracked());
    assert_eq!(harness.document().focused_node(), None);

    harness.touch(TouchPhase::End, end);
    assert!(
        harness
            .document()
            .api::<unstyled::ScrollApi>(scroll)
            .animating()
    );
    assert!(!hovered.get_untracked());
}
