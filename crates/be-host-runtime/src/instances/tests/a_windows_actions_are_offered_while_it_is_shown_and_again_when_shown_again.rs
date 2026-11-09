use super::*;

#[test]
fn a_windows_actions_are_offered_while_it_is_shown_and_again_when_shown_again() {
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
        size: Size {
            width: 200.0,
            height: 100.0,
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
    let offered = |instances: &Instances| {
        instances
            .instance_actions(INSTANCE, ViewId::MAIN)
            .into_iter()
            .filter(|(view, _)| *view == window)
            .map(|(_, action)| action.id)
            .collect::<Vec<_>>()
    };
    instances.mount(INSTANCE, View::window(window), Uuid::nil(), role, &block_types);
    assert_eq!(offered(&instances), ["tidy"], "a shown window offers its actions");

    instances.unmount(INSTANCE, window);
    assert!(
        offered(&instances).is_empty(),
        "a window the host stopped showing offers none"
    );
    assert_eq!(instances.action_screen(INSTANCE, window, "tidy"), None);

    instances.mount(INSTANCE, View::window(window), Uuid::nil(), role, &block_types);
    assert_eq!(
        offered(&instances),
        ["tidy"],
        "shown again, it offers them again without the app sending them anew"
    );
    assert_eq!(instances.action_screen(INSTANCE, window, "tidy"), Some(screen));
}
