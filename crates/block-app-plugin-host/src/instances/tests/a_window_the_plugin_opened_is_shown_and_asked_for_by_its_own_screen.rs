use super::*;

#[test]
fn a_window_the_plugin_opened_is_shown_and_asked_for_by_its_own_screen() {
    let mut instances = placed();
    let window = ViewId(9);
    let screen = ScreenId::opened(1);
    let layout = ScreenLayout {
        screens: vec![block_plugin_api::ScreenPlacement {
            screen,
            instance: INSTANCE,
            view: window,
            surface: 4,
            width: 200,
            height: 100,
            scale_factor_millis: 1000,
            raster_millis: 1000,
        }],
    };
    let open = WindowOpen {
        view: window,
        screen,
        opener: block_plugin_api::ViewRef {
            instance: INSTANCE,
            view: ViewId::MAIN,
        },
        placement: block_plugin_api::WindowPlacement::AtPointer,
        size: Size {
            width: 200.0,
            height: 100.0,
        },
        role: block_plugin_api::WindowRole::Normal,
    };
    instances.adopt_window(&open, &layout);
    let report = FrameReport {
        screen,
        content: Some(block_plugin_api::Insets::default()),
        painted: vec![Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0))],
        floating: Vec::new(),
        floating_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        anchored: None,
    };
    instances.set_frame_reports(vec![report.clone()]);
    assert_eq!(
        instances.frame_report(INSTANCE, window),
        Some(&report),
        "the report the window opened with is the window's"
    );

    let block_types = Arc::new(crate::PluginCatalog::default());
    let role = InstanceRole::Editor(
        Uuid::nil(),
        crate::EditorBlock {
            id: Uuid::nil(),
            block_type: Uuid::nil(),
            view_block: None,
        },
    );
    instances.mount(
        INSTANCE,
        View::window(window),
        Uuid::nil(),
        role,
        &block_types,
    );
    assert_eq!(
        instances.screen_id(INSTANCE, window),
        Some(screen),
        "the host shows the window by the screen the plugin opened it with"
    );
    instances.place_mounted(
        INSTANCE,
        window,
        None,
        vec2(200.0, 100.0),
        Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0)),
        1.0,
        1.0,
        true,
        None,
        None,
    );
    let asked = instances.next_screens(PASS).screens;
    assert!(
        asked
            .iter()
            .any(|request| request.screen == screen && request.view == window),
        "and asks for it by that screen: {asked:?}"
    );
}
