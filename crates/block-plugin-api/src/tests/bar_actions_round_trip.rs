use super::*;

#[test]
fn bar_actions_round_trip() {
    for action in [BarAction::CloseMore, BarAction::Details] {
        for message in [
            Message::Editor(EditorMessage::BarAction {
                instance: EditorInstanceId(3),
                action,
            }),
            Message::Editor(EditorMessage::ChildBar {
                instance: EditorInstanceId(2),
                region: EditorRegion::Frame,
                child: ChildId(7),
                action,
            }),
        ] {
            assert_eq!(
                decode_frame(&encode_frame(&message).unwrap()).unwrap(),
                message
            );
        }
    }
}
