use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;

#[test]
fn overscrolling_squishes_the_scrollbar_thumb_against_the_end() {
    let scroll = NodeRef::new();
    let held = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&held @sizing=ItemSize::Percent(100.0)>
                    <Frame height=1200.0 />
                </Scroll>
            </List>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let thumb = |harness: &Harness| {
        let bar = harness.document().children(scroll)[1];
        let track = harness.document().children(bar)[0];
        let list = harness.document().children(track)[0];
        (harness.rect(bar), harness.rect(harness.document().children(list)[1]))
    };
    let (bar, resting) = thumb(&harness);
    assert_eq!(resting.top(), bar.top());

    let x = harness.rect(scroll).center().x - 40.0;
    harness.touch(TouchPhase::Start, pos2(x, 40.0));
    harness.touch(TouchPhase::Move, pos2(x, 240.0));
    assert!(harness.document().scroll_overscroll(scroll) < 0.0);
    harness.frame(Vec::new());
    let (_, squished) = thumb(&harness);
    assert_eq!(squished.top(), bar.top(), "the thumb stays against the top");
    assert!(
        squished.height() < resting.height() - 5.0,
        "the thumb squishes while the content is pulled past its start: {} -> {}",
        resting.height(),
        squished.height()
    );

    harness.touch(TouchPhase::End, pos2(x, 240.0));
    harness.settle();
    let (_, rested) = thumb(&harness);
    assert_eq!(rested, resting, "the thumb springs back with the content");
}
