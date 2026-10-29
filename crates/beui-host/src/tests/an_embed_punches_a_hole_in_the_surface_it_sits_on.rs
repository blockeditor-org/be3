use super::*;
use crate::painter::Shape;
use crate::primitives::*;

#[test]
fn an_embed_punches_a_hole_in_the_surface_it_sits_on() {
    let _root = crate::reactive::RootScope::new();
    let slot = EmbedSlot::new();
    let solid = EmbedSlot::new();

    let document = build_node(move || {
        view! {
            <Frame fill=Color32::WHITE>
                <List spacing=0.0>
                    <Frame width=120.0 height=80.0>
                        <Embed slot={slot} />
                    </Frame>
                    <Frame width=120.0 height=80.0>
                        <Embed slot={solid} punch=false />
                    </Frame>
                </List>
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
        .main()
        .shapes()
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
