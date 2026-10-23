use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::sight::CULLING_MARGIN;

#[test]
fn an_embed_reports_the_rect_and_the_clip_it_was_laid_out_in() {
    let _root = crate::reactive::RootScope::new();
    let inside = EmbedSlot::new();
    let outside = EmbedSlot::new();

    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Offset @sizing=ItemSize::Fixed(120.0)>
                    <List spacing=0.0>
                        <Frame height=200.0>
                            <Embed slot={inside} />
                        </Frame>
                        <Frame height=CULLING_MARGIN />
                        <Frame height=200.0>
                            <Embed slot={outside} />
                        </Frame>
                        <Frame height=200.0>
                            <Embed slot={EmbedSlot::new()} />
                        </Frame>
                    </List>
                </Offset>
            </List>
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
