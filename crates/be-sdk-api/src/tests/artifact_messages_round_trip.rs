use super::*;

#[test]
fn artifact_messages_round_trip() {
    let opening = |kind| Opening {
        instance: EditorInstanceId(3),
        editor: [6; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind,
    };
    let messages = [
        opening(OpeningKind::Artifact(OpenArtifact {
            block_id: [1; 16],
            block_type: [2; 16],
            source: [7; 16],
            exporter: "png".to_owned(),
            data: b"{\"scale\":2}".to_vec(),
        })),
        opening(OpeningKind::Export(OpenExport {
            block_id: [7; 16],
            block_type: [8; 16],
            request: ExportRequest {
                exporter: "svg".to_owned(),
                settings: b"{}".to_vec(),
            },
        })),
    ];
    for message in messages {
        round_trips(&message);
    }
}
