use super::*;

#[test]
fn an_anchored_view_is_shown_from_the_report_that_opens_it_clipped_to_its_bounds() {
    let mut instances = layered(pos2(10.0, 10.0));
    assert!(instances.shown_anchored(INSTANCE, REGION).is_empty());

    let menu = Rect::from_min_size(pos2(350.0, 40.0), vec2(100.0, 80.0));
    report_anchored(&mut instances, vec![layer(1, 0, menu)]);

    let shown = instances.shown_anchored(INSTANCE, REGION);
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
    assert_eq!(
        (layer.surface, layer.screen),
        (7, ScreenId::opened(1)),
        "it is drawn from the screen the plugin opened for it"
    );

    report_anchored(&mut instances, Vec::new());
    assert!(
        instances.shown_anchored(INSTANCE, REGION).is_empty(),
        "a view whose screen leaves the plugin's layout is closed"
    );
}
