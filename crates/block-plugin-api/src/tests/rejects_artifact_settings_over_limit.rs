use super::*;

#[test]
fn rejects_artifact_settings_over_limit() {
    let message = Message::Editor(EditorMessage::OpenArtifact {
        instance: EditorInstanceId(1),
        editor: [6; 16],
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        data: vec![0; MAX_OPAQUE_DESCRIPTOR_BYTES + 1],
    });
    assert_eq!(
        encode_frame(&message),
        Err(DecodeError::LimitExceeded("artifact settings"))
    );
}
