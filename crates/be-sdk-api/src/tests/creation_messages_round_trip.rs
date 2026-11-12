use super::*;

#[test]
fn creation_messages_round_trip() {
    let message = Opening {
        instance: EditorInstanceId(2),
        editor: [1; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind: OpeningKind::Creation(OpenCreation {
            template: "title-slide".into(),
        }),
    };
    round_trips(&message);
}
