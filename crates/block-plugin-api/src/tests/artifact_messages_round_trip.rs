use super::*;

#[test]
fn artifact_messages_round_trip() {
    let messages = [Opening::Artifact(OpenArtifact {
        instance: EditorInstanceId(3),
        editor: [6; 16],
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        data: b"{\"scale\":2}".to_vec(),
    })];
    for message in messages {
        round_trips(&message);
    }
}
