use super::*;

#[test]
fn a_resize_leaves_children_where_their_frame_painted_them() {
    let mut instances = placed();
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(1)));
    instances.set_children(child(
        Rect::from_min_size(pos2(0.0, 50.0), vec2(20.0, 20.0)),
        false,
    ));
    let screen = instances
        .screen_id(INSTANCE, REGION)
        .expect("the frame has a screen");
    instances.set_windows(vec![ScreenWindow {
        screen,
        window: WindowRequests {
            ime: Some(ImeArea {
                rect: Rect::from_min_size(pos2(0.0, 50.0), vec2(100.0, 20.0)),
                cursor: Rect::from_min_size(pos2(4.0, 50.0), vec2(1.0, 20.0)),
                text: None,
                keyboard: block_plugin_api::Keyboard::Shown { asks: 1 },
            }),
            ..WindowRequests::default()
        },
    }]);
    instances.confirm(1, &HashSet::new());

    let shrunk = vec2(100.0, 60.0);
    resize(&mut instances, shrunk, 1.0);
    assert_eq!(configure(&mut instances), Some(PluginCall::Configure(2)));
    let rect = Rect::from_min_size(pos2(10.0, 10.0), shrunk);
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 60.0), vec2(20.0, 20.0)),
        "a frame painted at 100x100 is shown at its own size from the region's top-left, \
         cropped to the 100x60 the region now has, so its children are not squeezed either"
    );
    assert_eq!(
        instances.ime(INSTANCE, REGION, rect).map(|ime| ime.rect),
        Some(Rect::from_min_size(pos2(10.0, 60.0), vec2(100.0, 20.0))),
        "the text input area stays where the frame painted it"
    );

    instances.confirm(2, &HashSet::new());
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 60.0), vec2(20.0, 20.0)),
        "once the plugin answers the resize, its children are still placed one to one"
    );
}
