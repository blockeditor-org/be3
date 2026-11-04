use super::*;
use crate::primitives::*;

#[test]
fn an_embed_reports_a_rect_on_the_pixel_grid() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame aspect_ratio=1.0>
                        <Embed slot={slot} />
                    </Frame>
                </ListItem>
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
