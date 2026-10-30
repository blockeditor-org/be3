use super::*;

#[test]
fn a_view_attached_to_an_edge_stays_on_it_when_the_view_it_hangs_from_moves() {
    let mut instances = layered(pos2(10.0, 10.0));
    let sheet = Rect::from_min_size(pos2(0.0, 200.0), vec2(400.0, 100.0));
    let mut attached = layer(1, 0, sheet);
    attached.placement.anchor =
        block_plugin_api::ViewAnchor::Edge(block_plugin_api::ViewEdge::Bottom);
    report_anchored(&mut instances, vec![attached]);

    place_layered(&mut instances, pos2(40.0, 30.0), window());
    assert_eq!(
        shown_rects(&instances),
        vec![sheet],
        "a sheet hangs from the edge of the bounds, not from its region, so a move leaves it there"
    );
}
