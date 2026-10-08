use super::*;

#[test]
fn host_window_messages_round_trip() {
    let window = HostWindow {
        id: HostWindowId(7),
        title: "Terminal".into(),
        app_id: "foot".into(),
        parent: Some(HostWindowId(3)),
        size: Size {
            width: 640.0,
            height: 480.0,
        },
        fullscreen: None,
    };
    for message in [
        Message::Editor(EditorMessage::Linux {
            instance: EditorInstanceId(3),
            message: LinuxMessage::Windows(vec![window.clone()]),
        }),
        Message::Editor(EditorMessage::CloseWindow {
            instance: EditorInstanceId(3),
            window: window.id,
        }),
        Message::Children(ChildPlacements {
            instance: EditorInstanceId(3),
            region: EditorRegion::Frame,
            generation: 2,
            size: Size {
                width: 800.0,
                height: 600.0,
            },
            children: vec![ChildPlacement {
                child: ChildId(1),
                content: ChildContent::Window(window.id),
                rect: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 480.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
            }],
            occluders: Vec::new(),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
