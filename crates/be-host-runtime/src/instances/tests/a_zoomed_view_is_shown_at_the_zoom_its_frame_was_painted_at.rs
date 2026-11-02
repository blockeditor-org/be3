use super::*;

#[test]
fn a_zoomed_view_is_shown_at_the_zoom_its_frame_was_painted_at() {
    let mut instances = placed();
    resize(&mut instances, vec2(50.0, 50.0), 2.0);
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(1)));
    instances.set_children(child(
        Rect::from_min_size(pos2(0.0, 25.0), vec2(10.0, 10.0)),
        false,
    ));
    instances.confirm(1, &HashSet::default());
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);

    resize(&mut instances, vec2(25.0, 25.0), 4.0);
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(2)));
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 60.0), vec2(20.0, 20.0)),
        "a frame painted at twice its size is shown at that until the plugin paints the new zoom"
    );

    instances.confirm(2, &HashSet::default());
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 110.0), vec2(40.0, 40.0)),
        "a frame painted at the new zoom is shown at it"
    );
}
