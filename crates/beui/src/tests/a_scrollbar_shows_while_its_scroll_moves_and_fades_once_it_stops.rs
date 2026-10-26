use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::unstyled::{SCROLLBAR_FADE_OUT, SCROLLBAR_LINGER};
use crate::primitives::List;

#[test]
fn a_scrollbar_shows_while_its_scroll_moves_and_fades_once_it_stops() {
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
    let output = harness.frame(Vec::new());
    let track = track_within(&harness, scrollbar_of(&harness, scroll.get()));
    assert!(
        !thumb_painted(&output, track),
        "a resting scroll shows no bar"
    );

    let over = harness.rect(scroll.get()).center();
    harness.scroll_at(over, vec2(0.0, -40.0), Modifiers::NONE);
    let output = harness.frame(Vec::new());
    assert!(thumb_painted(&output, track), "scrolling shows the bar");

    harness.advance(SCROLLBAR_LINGER / 2);
    let output = harness.frame(Vec::new());
    assert!(thumb_painted(&output, track), "the bar lingers a moment");

    harness.advance(SCROLLBAR_LINGER / 2 + SCROLLBAR_FADE_OUT);
    let output = harness.frame(Vec::new());
    assert!(!thumb_painted(&output, track), "then it fades out");
    let output = harness.frame(Vec::new());
    assert!(
        !output.repaint && output.repaint_after > Duration::ZERO,
        "a faded bar asks for no more frames"
    );
}
