use super::*;

#[test]
fn a_window_childs_status_is_its_programs() {
    let mut status = HostChildStatus {
        child: ChildId(7),
        available: true,
        intrinsic: None,
        aspect_ratio: None,
        active: false,
        interaction: be_sdk_api::InteractionMode::Live,
        capabilities: be_sdk_api::EditorCapabilities::default(),
        resize: be_sdk_api::ResizeMode::Both,
        error: None,
        creation: None,
        preview: ChildPreview::Hosted,
        dialog: DialogStatus::default(),
        picked: None,
        card: false,
        title: None,
        icon: None,
        group: None,
        raised: 0,
        rung: 0,
        fullscreen: false,
        notice: None,
        menu_asked: None,
        menu: Vec::new(),
    };
    let shown = WindowStatus {
        name: "Files".to_owned(),
        title: "Files \u{2014} Downloads".to_owned(),
        icon: Some(be_sdk_api::Picture {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        }),
        group: "org.gnome.Nautilus".to_owned(),
        raised: 3,
        rung: 2,
        fullscreen: true,
        responding: true,
        notice: Some("Press Super+Escape to take the keyboard back".to_owned()),
        menu: Some(be_sdk_api::MenuAsk {
            asked: 1,
            at: Pos2::new(10.0, 20.0),
        }),
        size: vec2(640.0, 480.0),
        drawn: vec2(400.0, 300.0),
        min_size: Vec2::ZERO,
    };
    with_window(&mut status, shown.clone());
    assert_eq!(status.title.as_deref(), Some("Files \u{2014} Downloads"));
    assert_eq!(status.icon, shown.icon);
    assert_eq!(status.group.as_deref(), Some("org.gnome.Nautilus"));
    assert_eq!((status.raised, status.rung), (3, 2));
    assert!(status.fullscreen);
    assert_eq!(status.notice, shown.notice);
    assert_eq!(status.menu_asked, shown.menu);
    assert_eq!(
        status.intrinsic,
        Some(vec2(640.0, 480.0)),
        "a window's intrinsic size is the one its program picked"
    );

    with_window(
        &mut status,
        WindowStatus {
            group: String::new(),
            size: Vec2::ZERO,
            ..shown
        },
    );
    assert_eq!(status.group, None, "a window of no program is in no group");
    assert_eq!(status.intrinsic, None, "nor has one before it draws");
}
