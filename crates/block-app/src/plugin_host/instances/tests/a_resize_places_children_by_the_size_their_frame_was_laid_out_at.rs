use super::*;

fn resize(instances: &mut Instances, size: Vec2) {
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    instances.report(
        INSTANCE,
        REGION,
        Uuid::nil(),
        role,
        &Arc::new(block_plugin_api::Catalog::default()),
        Some(block_plugin_api::FrameSpec::default()),
        size,
        Rect::from_min_size(Pos2::ZERO, size),
        1.0,
        PASS,
    );
}

#[test]
fn a_resize_places_children_by_the_size_their_frame_was_laid_out_at() {
    let mut instances = placed();
    assert_eq!(
        configure(&mut instances),
        Some(Message::Configure { serial: 1 })
    );
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
    resize(&mut instances, shrunk);
    assert_eq!(
        configure(&mut instances),
        Some(Message::Configure { serial: 2 })
    );
    let rect = Rect::from_min_size(pos2(10.0, 10.0), shrunk);
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 40.0), vec2(20.0, 12.0)),
        "a frame laid out at 100x100 is shown in the 100x60 the region now has"
    );
    assert_eq!(
        instances.ime(INSTANCE, REGION, rect).map(|ime| ime.rect),
        Some(Rect::from_min_size(pos2(10.0, 40.0), vec2(100.0, 12.0))),
        "the text input area moves with the frame it was laid out in"
    );

    instances.confirm(2, &HashSet::new());
    let (children, _) = instances.host_children(INSTANCE, REGION, rect, rect, None);
    assert_eq!(
        children[0].rect,
        Rect::from_min_size(pos2(10.0, 60.0), vec2(20.0, 20.0)),
        "once the plugin answers the resize, nothing is stretched"
    );
}
