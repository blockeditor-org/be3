use super::*;

#[test]
fn a_menu_pick_lets_an_instance_open_the_palette_only_in_the_frames_right_after_it() {
    let mut instances = placed();
    let window = ViewId(9);
    let screen = ScreenId::opened(1);
    let layout = ScreenLayout {
        screens: vec![be_sdk_api::ScreenPlacement {
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
        opener: be_sdk_api::ViewRef {
            instance: INSTANCE,
            view: ViewId::MAIN,
        },
        placement: be_sdk_api::WindowPlacement::AtPointer,
        size: Vec2 {
            x: 200.0,
            y: 100.0,
        },
        level: be_sdk_api::WindowLevel::Normal,
    };
    instances.adopt_window(&open, &layout);
    let report = FrameReport {
        screen,
        content: Some(be_sdk_api::Insets::default()),
        painted: vec![Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0))],
        floating: Vec::new(),
        floating_painted: Vec::new(),
        marks: Vec::new(),
        marks_painted: Vec::new(),
        scrims: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: None,
        actions: Some(vec![be_sdk_api::DescribedAction {
            id: "tidy".to_owned(),
            label: "Tidy".to_owned(),
            glyph: None,
            shortcut: None,
            enabled: true,
            checked: None,
            live: true,
        }]),
        anchored: None,
        work_area: None,
        backdrop: None,
    };
    instances.set_frame_reports(vec![report]);
    let block_types = Arc::new(crate::AppCatalog::default());
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
    assert!(
        !instances.may_open_palette(INSTANCE),
        "an instance without the keyboard may not open the palette"
    );

    instances.note_menu_pick(INSTANCE);
    assert!(
        instances.may_open_palette(INSTANCE),
        "it may right after the person picked from its menu"
    );
    assert!(
        !instances.may_open_palette(INSTANCE),
        "but only once per pick"
    );

    instances.note_menu_pick(INSTANCE);
    for _ in 0..3 {
        instances.set_frame_reports(Vec::new());
    }
    assert!(
        !instances.may_open_palette(INSTANCE),
        "nor frames after a pick of anything else, from the background"
    );
}
