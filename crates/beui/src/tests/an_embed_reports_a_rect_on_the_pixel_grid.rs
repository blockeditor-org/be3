use super::*;
use crate::reactive::{Embed, EmbedSlot, ItemSize, List, build, view};

#[test]
fn an_embed_reports_a_rect_on_the_pixel_grid() {
    let slot = EmbedSlot::new();
    let embedded = slot.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) aspect_ratio=1.0>
                    <Embed slot={embedded} />
                </Frame>
            </List>
        }
    });

    let mut harness = Harness::sized(document, Vec2::new(101.0, 100.0));
    harness.frame(Vec::new());

    let placed = slot.placement().expect("the embed was laid out");
    assert_eq!(placed.rect.min.x, placed.rect.min.x.round());
    assert_eq!(placed.rect.max.x, placed.rect.max.x.round());
    assert_eq!(placed.rect.width(), 100.0);
}
