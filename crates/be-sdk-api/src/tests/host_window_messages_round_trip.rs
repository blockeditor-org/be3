use super::*;

#[test]
fn host_window_messages_round_trip() {
    let window = HostWindow {
        id: HostWindowId(7),
        kind: WindowKind::Display,
        parent: Some(HostWindowId(3)),
        modal: true,
        program_id: "foot".into(),
        origin: WindowOrigin {
            program: Some("foot.desktop".into()),
            command: vec!["foot".into(), "--server".into()],
            launch: Some(9),
        },
        size: Some(Vec2::new(640.0, 480.0)),
        maximized: true,
    };
    watched_round_trips::<WindowsWatch>(vec![window.clone()]);
    {
        let message = committed(ChildPlacements {
            instance: EditorInstanceId(3),
            view: ViewId::MAIN,
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
                fit: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
                dialog: None,
                docks: None,
                anchor: None,
                fullscreen: false,
            }],
            occluders: Vec::new(),
        });
        round_trips(&message);
    }
}
