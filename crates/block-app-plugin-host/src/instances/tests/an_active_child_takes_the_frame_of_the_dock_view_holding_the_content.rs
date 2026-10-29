use super::*;
use block_plugin_api::{ChildId, ChildLayer, ChildMode, ChildPlacement, ChildPlacements, Rect};

const STAGE: ViewId = ViewId(5);

fn report(instances: &mut Instances, view: ViewId, content: Option<block_plugin_api::Insets>) {
    let screen = instances
        .screen_id(INSTANCE, view)
        .expect("the view has a screen");
    instances.set_frame_reports(vec![block_plugin_api::FrameReport {
        screen,
        content,
        painted: Vec::new(),
        floating: Vec::new(),
        floating_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        anchored: Vec::new(),
    }]);
}

#[test]
fn an_active_child_takes_the_frame_of_the_dock_view_holding_the_content() {
    let mut instances = placed();
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let role = InstanceRole::Editor(
        Uuid::nil(),
        EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    let catalog = Arc::new(crate::PluginCatalog::default());
    instances.mount(INSTANCE, View::dock(STAGE), Uuid::nil(), role, &catalog);
    instances.report(
        INSTANCE,
        View::dock(STAGE),
        Uuid::nil(),
        role,
        &catalog,
        None,
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(INSTANCE, STAGE, Placement { rect, clip: rect });
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);
    report(&mut instances, ViewId::MAIN, None);
    report(
        &mut instances,
        STAGE,
        Some(block_plugin_api::Insets::default()),
    );
    assert_eq!(
        instances.content_view(INSTANCE),
        STAGE,
        "the dock view that reports content holds it"
    );
    instances.set_children(ChildPlacements {
        instance: INSTANCE,
        view: STAGE,
        children: vec![ChildPlacement {
            child: ChildId(1),
            content: ChildContent::Block {
                block_id: [1; 16],
                block_type: [0; 16],
                view_block: None,
                editor: None,
            },
            rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(20.0, 20.0)),
            clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(100.0, 100.0)),
            own_frame: false,
            top_bar: block_plugin_api::TopBar::Hidden,
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Active,
            intrinsic: None,
            rotation: 0.0,
            opacity: 1.0,
            follows_view: false,
            dialog: None,
            docks: None,
            anchored: None,
        }],
        occluders: Vec::new(),
    });
    assert_eq!(
        instances.frame_child(INSTANCE),
        Some(Uuid::from_bytes([1; 16])),
        "an active child there takes the frame"
    );

    instances.revoke_frame_child(INSTANCE);
    assert!(
        instances.frame_child(INSTANCE).is_none(),
        "and leaving it hands that view's frame back"
    );
}
