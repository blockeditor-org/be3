use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::unstyled::{SCROLLBAR_FADE_OUT, SCROLLBAR_LINGER};

#[test]
fn pointing_at_the_edge_of_a_scroll_brings_back_its_scrollbar_with_its_track() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=2000.0 />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    let theme = styled::Theme::DARK;
    let output = harness.frame(Vec::new());
    let bar = harness.rect(scrollbar_of(&harness, scroll.get()));
    let track = track_within(&harness, scrollbar_of(&harness, scroll.get()));
    assert!(!thumb_painted(&output, track));
    assert!(!painted_rect(&output, track, theme.surface_raised));

    harness.frame(vec![Input::PointerMoved(pos2(
        bar.center().x,
        bar.bottom() - 4.0,
    ))]);
    let output = harness.frame(Vec::new());
    assert!(
        thumb_painted(&output, track),
        "pointing at the edge shows the bar"
    );
    assert!(
        painted_rect(&output, track, theme.surface_raised),
        "and its track, under the pointer"
    );

    harness.advance(SCROLLBAR_LINGER * 3);
    let output = harness.frame(Vec::new());
    assert!(
        thumb_painted(&output, track),
        "the bar stays while the pointer is on it"
    );

    harness.frame(vec![Input::PointerMoved(
        harness.rect(scroll.get()).center(),
    )]);
    let output = harness.frame(Vec::new());
    assert!(
        !painted_rect(&output, track, theme.surface_raised),
        "the track goes with the pointer"
    );
    assert!(thumb_painted(&output, track), "and the thumb lingers");

    harness.advance(SCROLLBAR_LINGER + SCROLLBAR_FADE_OUT);
    let output = harness.frame(Vec::new());
    assert!(!thumb_painted(&output, track), "before it fades out");
}

fn painted_rect(output: &crate::FrameOutput, rect: Rect, color: Color32) -> bool {
    output.main().shapes().iter().any(|shape| {
        matches!(shape, crate::Shape::Rect { rect: painted, color: fill, .. } if *painted == rect && *fill == color)
    })
}
