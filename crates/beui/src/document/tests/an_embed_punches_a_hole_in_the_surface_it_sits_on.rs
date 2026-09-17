use super::*;
use crate::painter::Shape;
use crate::reactive::{Embed, EmbedSlot, Frame, build, view};

#[test]
fn an_embed_punches_a_hole_in_the_surface_it_sits_on() {
    let slot = EmbedSlot::new();
    let embed = slot.clone();
    let solid = EmbedSlot::new();
    let covered = solid.clone();

    let document = build(move || {
        view! {
            <Frame color=Color32::WHITE>
                <Column spacing=0.0>
                    <Embed slot={embed} width=120.0 height=80.0 />
                    <Embed slot={covered} width=120.0 height=80.0 punch=false />
                </Column>
            </Frame>
        }
    });

    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());

    let hole = slot.placement().expect("the first embed was laid out").rect;
    let kept = solid
        .placement()
        .expect("the second embed was laid out")
        .rect;
    let punched: Vec<_> = output
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Punch { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect();

    assert_eq!(
        punched,
        vec![hole],
        "only the embed that punches should have cut the surface, not the one at {kept:?}"
    );
}
