use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

#[test]
fn a_sheet_escapes_attached_to_the_bottom_of_its_layer_space() {
    let _root = RootScope::new();
    let body = NodeRef::new();
    let document = build_node(move || {
        view! {
            <ModalSheet open=true fit=true on_close={|| {}}>
                <Frame @node_ref=&body height=120.0 />
            </ModalSheet>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 300.0));
    give_layer_space(&mut harness);
    harness.settle();

    let layers = harness.document().escaped_layers();
    assert_eq!(layers.len(), 1, "the sheet escapes");
    let layer = &layers[0];
    assert_eq!(layer.placement, LayerPlacement::Edge(LayerEdge::Bottom));
    assert_eq!(layer.rect, LAYER_SPACE.bounds, "the sheet's layer is the layer space");
    let sheet = LAYER_SPACE.to_host.rect(harness.rect(body.get()));
    assert!(
        sheet.bottom() > LAYER_SPACE.to_host.point(pos2(0.0, 300.0)).y,
        "the sheet sits against the bottom of the layer space, below its region: {sheet:?}"
    );
}
