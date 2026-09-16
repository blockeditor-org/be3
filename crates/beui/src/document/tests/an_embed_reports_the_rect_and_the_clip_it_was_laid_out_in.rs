use super::*;
use crate::reactive::{Embed, EmbedSlot, ItemSize, Scroll, build, intrinsic, view};

#[test]
fn an_embed_reports_the_rect_and_the_clip_it_was_laid_out_in() {
    let inside = EmbedSlot::new();
    let outside = EmbedSlot::new();
    let first = inside.clone();
    let second = outside.clone();

    let document = build(move || {
        let rows = vec![
            intrinsic(view! {
                <Embed slot={first} height=200.0 />
            }),
            intrinsic(view! {
                <Embed slot={second} height=200.0 />
            }),
            intrinsic(view! {
                <Embed slot={EmbedSlot::new()} height=200.0 />
            }),
        ];
        view! {
            <Column spacing=0.0>
                <Scroll @sizing=ItemSize::Fixed(120.0) children={rows} />
            </Column>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let placed = inside.placement().expect("the first embed was laid out");
    assert_eq!(placed.rect.height(), 200.0);
    assert_eq!(placed.clip.height(), 120.0);
    assert!(
        harness
            .document()
            .node_rect(inside.node().unwrap())
            .is_some(),
        "the first embed has a laid out rect"
    );
    assert!(
        harness
            .document()
            .node_rect(outside.node().unwrap())
            .is_none(),
        "an embed scrolled out of view is not laid out"
    );
}
