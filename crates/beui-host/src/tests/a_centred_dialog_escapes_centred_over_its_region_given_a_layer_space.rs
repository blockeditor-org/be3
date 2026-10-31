use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_centred_dialog_escapes_centred_over_its_region_given_a_layer_space() {
    let _root = crate::reactive::RootScope::new();
    let surface = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog open=true kind=DialogKind::Alert title="Delete?" on_dismiss={|| {}}>
                    <Text color=Color32::WHITE @node_ref={&surface}>"Body"</Text>
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    give_layer_space(&mut harness);
    harness.frame(Vec::new());

    let layers = harness.document().escaped_layers();
    assert_eq!(
        layers.len(),
        1,
        "an alert escapes its region, so whatever the host shows over the region stays beneath it"
    );
    let region = Rect::from_min_size(LAYER_SPACE.to_host.origin, VIEWPORT);
    let card = layers[0].rect;
    assert!(
        (card.center().x - region.center().x).abs() < 2.0
            && (card.center().y - region.center().y).abs() < 2.0,
        "it is centred over its region, not over the host: {card:?} {region:?}"
    );
}
