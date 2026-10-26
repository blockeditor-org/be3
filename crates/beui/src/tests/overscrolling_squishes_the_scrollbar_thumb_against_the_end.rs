use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;

#[test]
fn overscrolling_squishes_the_scrollbar_thumb_against_the_end() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=1200.0 />
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let thumb = |harness: &mut Harness| {
        let output = harness.frame(Vec::new());
        let track = track_within(harness, scrollbar_of(harness, scroll));
        (track, painted_thumb(&output, track))
    };
    let edge = harness.rect(scrollbar_of(&harness, scroll)).center();
    harness.frame(vec![Input::PointerMoved(edge)]);
    let (bar, resting) = thumb(&mut harness);
    harness.frame(vec![Input::PointerLeft]);
    assert_eq!(resting.top(), bar.top());

    let x = harness.rect(scroll).center().x - 40.0;
    harness.touch(TouchPhase::Start, pos2(x, 40.0));
    harness.touch(TouchPhase::Move, pos2(x, 240.0));
    assert!(harness.document().scroll_overscroll(scroll) < 0.0);
    let (_, squished) = thumb(&mut harness);
    assert_eq!(squished.top(), bar.top(), "the thumb stays against the top");
    assert!(
        squished.height() < resting.height() - 5.0,
        "the thumb squishes while the content is pulled past its start: {} -> {}",
        resting.height(),
        squished.height()
    );

    harness.touch(TouchPhase::End, pos2(x, 240.0));
    harness.settle();
    let (_, rested) = thumb(&mut harness);
    assert_eq!(rested, resting, "the thumb springs back with the content");
}
