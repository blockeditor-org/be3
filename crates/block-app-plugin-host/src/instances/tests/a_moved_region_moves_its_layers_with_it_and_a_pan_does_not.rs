use super::*;

#[test]
fn a_moved_region_moves_its_layers_with_it_and_a_pan_does_not() {
    let mut instances = layered(pos2(10.0, 10.0));
    let menu = Rect::from_min_size(pos2(50.0, 50.0), vec2(60.0, 40.0));
    report_layers(&mut instances, vec![layer(1, 0, menu)]);

    instances.set_view(
        INSTANCE,
        EditorView {
            rect: Rect::from_min_size(pos2(-30.0, 20.0), SIZE),
            scale: 1.0,
        },
    );
    lay_out(&mut instances);
    assert_eq!(
        shown_rects(&instances),
        vec![menu],
        "panning the region's camera leaves its layers where the plugin put them"
    );

    place_layered(&mut instances, pos2(40.0, 30.0), window());
    let moved = menu.translate(vec2(30.0, 20.0));
    assert_eq!(
        shown_rects(&instances),
        vec![moved],
        "a region moved before its plugin lays out again carries its layers with it"
    );

    lay_out(&mut instances);
    report_layers(&mut instances, vec![layer(1, 0, menu)]);
    assert_eq!(
        shown_rects(&instances),
        vec![moved],
        "a move alone sends the plugin no new place, and its layers keep moving with the region"
    );

    assert!(
        instances.refresh_layers(INSTANCE, REGION),
        "input to the region sends it where it now is"
    );
    lay_out(&mut instances);
    report_layers(&mut instances, vec![layer(1, 0, moved)]);
    assert_eq!(
        shown_rects(&instances),
        vec![moved],
        "a layer laid out against the region's new place is not moved again"
    );
    assert!(!instances.refresh_layers(INSTANCE, REGION));
}
