use super::*;

#[test]
fn host_panel_messages_round_trip() {
    for panel in HostPanel::ALL {
        for message in [
            Message::Editor(EditorMessage::ShowPanel {
                instance: EditorInstanceId(3),
                panel,
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
                    content: ChildContent::Host(panel),
                    rect: ChildRect {
                        x: 40.0,
                        y: 40.0,
                        width: 520.0,
                        height: 420.0,
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
}
