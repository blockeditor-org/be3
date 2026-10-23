use super::*;

#[test]
fn web_view_messages_round_trip() {
    for message in [
        Message::Children(ChildPlacements {
            instance: EditorInstanceId(5),
            region: EditorRegion::Frame,
            generation: 1,
            size: Size {
                width: 640.0,
                height: 480.0,
            },
            children: vec![ChildPlacement {
                child: ChildId(1),
                content: ChildContent::WebView(WebViewId(2)),
                rect: Rect::from_min_size(Pos2::new(4.0, 8.0), Vec2::new(320.0, 240.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(640.0, 480.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Passive,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_camera: false,
            }],
            occluders: Vec::new(),
            camera: None,
        }),
        Message::Editor(EditorMessage::WebViewCommand {
            instance: EditorInstanceId(5),
            web_view: WebViewId(2),
            command: WebViewCommand::Open("https://example.com/".into()),
        }),
        Message::Editor(EditorMessage::WebViewCommand {
            instance: EditorInstanceId(5),
            web_view: WebViewId(2),
            command: WebViewCommand::Reload,
        }),
        Message::Editor(EditorMessage::WebViewEvent {
            instance: EditorInstanceId(5),
            web_view: WebViewId(2),
            event: WebViewEvent::History(-1),
        }),
        Message::Editor(EditorMessage::WebViewEvent {
            instance: EditorInstanceId(5),
            web_view: WebViewId(2),
            event: WebViewEvent::Title("Example Domain".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
