use super::*;

#[test]
fn shell_dialog_and_access_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::ShowDialog {
            instance: EditorInstanceId(2),
            block_id: [3; 16],
            dialog: ShellDialog::Share,
        }),
        Message::Editor(EditorMessage::SetAccess {
            instance: EditorInstanceId(2),
            block_id: [3; 16],
            account: [4; 16],
            access: AccessLevel::View,
        }),
        Message::Editor(EditorMessage::Request {
            instance: EditorInstanceId(2),
            request_id: 7,
            request: HostRequest::ListAccess([3; 16]),
        }),
        Message::Editor(EditorMessage::Replied {
            instance: EditorInstanceId(2),
            request_id: 7,
            reply: HostReply::AccessListed(AccessListing::Listed(vec![AccessGrant {
                account: [4; 16],
                email: "friend@example.org".into(),
                display_name: "Friend".into(),
                administrator: false,
                granted: Some(AccessLevel::View),
                effective: AccessLevel::View,
            }])),
        }),
        Message::Children(ChildPlacements {
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            generation: 1,
            size: Size {
                width: 360.0,
                height: 240.0,
            },
            children: vec![ChildPlacement {
                child: ChildId(6),
                content: ChildContent::ArtifactSettings { block_id: [3; 16] },
                rect: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 96.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 360.0,
                    height: 240.0,
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
    ];
    for message in messages {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
