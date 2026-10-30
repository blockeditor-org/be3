use super::*;

#[test]
fn children_laid_out_against_an_older_view_are_moved_to_the_current_one() {
    let mut instances = placed();
    let laid = EditorView {
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
        scale: 1.0,
    };
    instances.set_view(INSTANCE, laid);
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(1)));
    instances.set_children(child(
        Rect::from_min_size(pos2(10.0, 10.0), vec2(20.0, 20.0)),
        true,
    ));
    instances.confirm(1, &HashSet::default());

    let panned = EditorView {
        rect: Rect::from_min_size(pos2(30.0, 0.0), vec2(200.0, 200.0)),
        scale: 2.0,
    };
    instances.set_view(INSTANCE, panned);
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(2)));
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, Some(panned));
    assert_eq!(
        children[0].view_rect,
        Rect::from_min_size(pos2(60.0, 30.0), vec2(40.0, 40.0)),
        "until the plugin answers serial 2, the child it placed through the view at serial 1 \
         moves from that view to the current one"
    );

    instances.set_children(child(
        Rect::from_min_size(pos2(50.0, 20.0), vec2(40.0, 40.0)),
        true,
    ));
    instances.confirm(2, &HashSet::default());
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, Some(panned));
    assert_eq!(
        children[0].view_rect,
        Rect::from_min_size(pos2(60.0, 30.0), vec2(40.0, 40.0)),
        "a child laid out against the current view is not moved again"
    );
}
