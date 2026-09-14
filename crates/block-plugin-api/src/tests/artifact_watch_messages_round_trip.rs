use super::*;

use crate::ArtifactState;

#[test]
fn artifact_watch_messages_round_trip() {
    let message = Message::Editor(EditorMessage::WatchArtifacts {
        instance: EditorInstanceId(2),
        blocks: vec![[1; 16], [2; 16]],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
    let message = Message::Editor(EditorMessage::ArtifactStates {
        instance: EditorInstanceId(2),
        states: vec![
            ArtifactState {
                block_id: [1; 16],
                source_type: [4; 16],
                source: Some([5; 16]),
                summary: "512 by 512".into(),
                error: None,
                regenerating: true,
            },
            ArtifactState {
                block_id: [2; 16],
                source_type: [4; 16],
                source: None,
                summary: String::new(),
                error: Some("the settings could not be read".into()),
                regenerating: false,
            },
        ],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );
}
