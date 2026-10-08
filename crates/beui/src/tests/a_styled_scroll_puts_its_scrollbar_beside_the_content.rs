use super::*;
use crate::reactive::{ItemSize, List, NodeRef, build, view};
use crate::styled::Scroll;
use crate::styled::theme::{
    SCROLLBAR_INSET, SCROLLBAR_SPACING, SCROLLBAR_TRACK_WIDTH, SCROLLBAR_WIDTH,
};

const CONTENT_HEIGHT: f32 = 2000.0;

#[test]
fn a_styled_scroll_puts_its_scrollbar_beside_the_content() {
    let scroll = NodeRef::new();
    let held = scroll.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&held @sizing=ItemSize::Percent(100.0)>
                    <Frame height=CONTENT_HEIGHT />
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let children = harness.document().children(scroll.get());
    assert_eq!(children.len(), 2, "the scroll built something unexpected");
    let (whole, content, bar) = (
        harness.rect(scroll.get()),
        harness.rect(children[0]),
        harness.rect(children[1]),
    );
    assert_eq!(bar.width(), SCROLLBAR_WIDTH);
    assert_eq!(bar.left(), content.right() + SCROLLBAR_SPACING);
    assert_eq!(bar.right(), whole.right());
    assert_eq!(bar.height(), whole.height());

    let document = harness.document();
    let interactive = document.children(children[1])[0];
    let inset = document.children(interactive)[0];
    let track = document.children(inset)[0];
    let painted = harness.rect(track);
    assert_eq!(painted.left(), bar.left());
    assert_eq!(painted.width(), SCROLLBAR_TRACK_WIDTH);
    assert_eq!(painted.right(), whole.right() - SCROLLBAR_INSET);
    assert_eq!(painted.top(), whole.top() + SCROLLBAR_INSET);
    assert_eq!(painted.bottom(), whole.bottom() - SCROLLBAR_INSET);

    let list = document.children(track)[0];
    let thumb = harness.rect(document.children(list)[1]);
    let wanted = painted.height() * (whole.height() / CONTENT_HEIGHT);
    assert!(
        (thumb.height() - wanted).abs() <= 1.0,
        "thumb {} wanted {wanted}",
        thumb.height()
    );
}
