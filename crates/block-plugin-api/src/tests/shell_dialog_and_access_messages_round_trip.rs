use super::*;

#[test]
fn shell_dialog_and_access_messages_round_trip() {
    let messages = [
        Message::Editor(EditorMessage::ShowDialog {
            instance: EditorInstanceId(2),
            block_id: [3; 16],
            dialog: ShellDialog::Share,
        }),
        Message::Editor(EditorMessage::ShowDialog {
            instance: EditorInstanceId(2),
            block_id: [3; 16],
            dialog: ShellDialog::Rename {
                anchor: Some(Rect::from_min_size(
                    Pos2::new(4.0, 8.0),
                    Vec2::new(16.0, 24.0),
                )),
                within: Some([5; 16]),
            },
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
            reply: HostReply::AccessListed(AccessListing::Listed {
                grants: vec![AccessGrant {
                    account: [4; 16],
                    email: "friend@example.org".into(),
                    display_name: "Friend".into(),
                    administrator: false,
                    author: false,
                    granted: Some(AccessLevel::View),
                    inherited: AccessLevel::None,
                    effective: AccessLevel::Edit,
                    source: Some([5; 16]),
                    waiting: true,
                }],
                company: true,
                set_inside: 3,
            }),
        }),
        committed(ChildPlacements {
            instance: EditorInstanceId(1),
            region: EditorRegion::Frame,
            children: vec![ChildPlacement {
                child: ChildId(6),
                content: ChildContent::ArtifactSettings { block_id: [3; 16] },
                rect: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(320.0, 96.0)),
                clip: Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(360.0, 240.0)),
                own_frame: false,
                top_bar: TopBar::Hidden,
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic: None,
                rotation: 0.0,
                opacity: 1.0,
                follows_view: false,
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
