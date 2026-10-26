use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::theme::SCROLLBAR_WIDTH;
use crate::styled::{Size, Space};
use crate::primitives::List;

const CONTENT_HEIGHT: f32 = 2000.0;

#[test]
fn a_styled_scroll_lays_its_scrollbar_over_the_edge_of_its_content() {
    let _root = crate::reactive::RootScope::new();
    let scroll = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let children = harness
        .document()
        .children(layers_of(harness.document(), scroll.get()));
    assert_eq!(children.len(), 2, "the scroll built something unexpected");
    let (whole, content) = (harness.rect(scroll.get()), harness.rect(children[0]));
    let bar = harness.rect(scrollbar_of(&harness, scroll.get()));
    assert_eq!(content, whole, "the content keeps the whole width");
    assert_eq!(bar.width(), SCROLLBAR_WIDTH);
    assert_eq!(bar.right(), whole.right());
    assert_eq!(bar.height(), whole.height());

    let painted = track_within(&harness, scrollbar_of(&harness, scroll.get()));
    assert_eq!(painted.left(), bar.left());
    assert_eq!(painted.width(), Size::Track.px());
    assert_eq!(painted.right(), whole.right() - Space::Xs2.px());
    assert_eq!(painted.top(), whole.top() + Space::Xs2.px());
    assert_eq!(painted.bottom(), whole.bottom() - Space::Xs2.px());

    harness.scroll_at(whole.center(), vec2(0.0, -40.0), Modifiers::NONE);
    let output = harness.frame(Vec::new());
    let thumb = painted_thumb(&output, painted);
    let wanted = painted.height() * (whole.height() / CONTENT_HEIGHT);
    assert!(
        (thumb.height() - wanted).abs() <= 1.0,
        "thumb {} wanted {wanted}",
        thumb.height()
    );
}
