use super::*;

#[test]
fn children_of_a_dock_view_holding_the_content_follow_its_camera() {
    const TAB: ViewId = ViewId(7);
    let mut instances = placed();
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    instances.report(
        INSTANCE,
        View::dock(TAB),
        Uuid::nil(),
        InstanceRole::Editor(
            Uuid::nil(),
            EditorBlock {
                id: Uuid::nil(),
                block_type: Uuid::nil(),
                view_block: None,
            },
        ),
        &Arc::new(crate::PluginCatalog::default()),
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(INSTANCE, TAB, Placement { rect, clip: rect });
    let laid = EditorView {
        rect: Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
        scale: 1.0,
    };
    instances.set_view(INSTANCE, laid);
    configure(&mut instances);
    let mut placements = child(
        Rect::from_min_size(pos2(10.0, 10.0), vec2(20.0, 20.0)),
        true,
    );
    placements.view = TAB;
    instances.set_children(placements);
    instances.confirm(instances.serial(), &HashSet::default());

    let panned = EditorView {
        rect: Rect::from_min_size(pos2(0.0, -40.0), vec2(100.0, 100.0)),
        scale: 1.0,
    };
    instances.set_view(INSTANCE, panned);
    configure(&mut instances);
    let (children, _) = instances.host_children(INSTANCE, TAB, rect, rect, Some(panned));
    assert_eq!(
        children[0].view_rect,
        Rect::from_min_size(pos2(20.0, -20.0), vec2(20.0, 20.0)),
        "a pan moves a child of the dock view the camera is drawn in before the plugin answers, \
         as it does in the main view"
    );
}
