use super::*;

#[test]
fn open_messages_round_trip() {
    let message = Opening::Block(OpenBlock {
        instance: EditorInstanceId(9),
        editor: [7; 16],
        block_id: [1; 16],
        block_type: [2; 16],
        view_block: Some([6; 16]),
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    });
    round_trips(&message);
}
