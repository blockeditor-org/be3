use super::*;

#[test]
fn a_layer_is_shown_from_the_report_that_opens_it_clipped_to_its_bounds() {
    let mut instances = layered(pos2(10.0, 10.0));
    assert!(instances.layers(INSTANCE, REGION).is_empty());

    let menu = Rect::from_min_size(pos2(350.0, 40.0), vec2(100.0, 80.0));
    report_layers(
        &mut instances,
        vec![layer(1, block_plugin_api::LayerParent::Region, menu)],
    );

    let shown = instances.layers(INSTANCE, REGION);
    assert_eq!(shown.len(), 1, "the report that opens a layer shows it");
    let layer = &shown[0];
    assert_eq!(
        layer.rect, menu,
        "a layer is placed where the plugin put it"
    );
    assert_eq!(layer.bounds, window());
    let clipped = Rect::from_min_max(pos2(350.0, 40.0), pos2(400.0, 120.0));
    assert_eq!(
        (layer.painted.clone(), layer.floating.clone()),
        (vec![clipped], vec![clipped]),
        "what it paints and where it takes input stop at its bounds"
    );
    assert_eq!(layer.surface, 7);
    assert_eq!(instances.layer_surfaces(), vec![7]);

    report_layers(&mut instances, Vec::new());
    assert!(
        instances.layers(INSTANCE, REGION).is_empty(),
        "each report replaces the last one's layers"
    );
}
