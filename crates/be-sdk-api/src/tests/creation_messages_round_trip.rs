use super::*;

#[test]
fn creation_messages_round_trip() {
    let message = Opening::Creation(OpenCreation {
        instance: EditorInstanceId(2),
        editor: [1; 16],
        template: "title-slide".into(),
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
    });
    round_trips(&message);
}
