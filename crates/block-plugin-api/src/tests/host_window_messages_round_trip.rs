use super::*;

#[test]
fn host_window_messages_round_trip() {
    let window = HostWindow {
        id: HostWindowId(7),
        kind: WindowKind::Display,
        shown: false,
        title: "Terminal".into(),
        app_id: "foot".into(),
        parent: Some(HostWindowId(3)),
        modal: true,
        size: Vec2::new(640.0, 480.0),
        fullscreen: None,
        responding: true,
        focused: false,
        icon: Some(HostImage {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
        }),
        raised: 0,
        origin: WindowOrigin {
            program: Some("foot.desktop".into()),
            command: vec!["foot".into(), "--server".into()],
            launch: Some(9),
        },
        note: None,
    };
    watched_round_trips::<WindowsWatch>(vec![window.clone()]);
    for action in [
        WindowAction::Focus(window.id),
        WindowAction::Fullscreen {
            window: window.id,
            fullscreen: true,
        },
        WindowAction::Close(window.id),
        WindowAction::Show(window.id),
        WindowAction::Hide(window.id),
    ] {
        round_trips(&action);
    }
    {
        let message = committed(ChildPlacements {
            instance: EditorInstanceId(3),
            region: EditorRegion::Frame,
            children: vec![ChildPlacement {
                child: ChildId(1),
                content: ChildContent::Window(window.id),
                rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
                dialog: None,
            }],
            occluders: Vec::new(),
        });
        round_trips(&message);
    }
}
