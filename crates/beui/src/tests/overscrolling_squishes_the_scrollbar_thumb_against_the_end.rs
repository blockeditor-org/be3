use super::*;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;

#[test]
fn overscrolling_squishes_the_scrollbar_thumb_against_the_end() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build(move || {
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
        let bar = harness.document().children(scroll)[1];
        let interactive = harness.document().children(bar)[0];
        let inset = harness.document().children(interactive)[0];
        let track = harness.rect(harness.document().children(inset)[0]);
        (track, painted_thumb(&output, track))
    };
    let (bar, resting) = thumb(&mut harness);
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
