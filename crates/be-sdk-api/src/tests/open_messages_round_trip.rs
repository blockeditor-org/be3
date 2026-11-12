use super::*;

#[test]
fn open_messages_round_trip() {
    let message = Opening {
        instance: EditorInstanceId(9),
        editor: [7; 16],
        account_id: [3; 16],
        organization_id: [4; 16],
        client_id: [5; 16],
        kind: OpeningKind::Block(OpenBlock {
            block_id: [1; 16],
            block_type: [2; 16],
            view_block: Some([6; 16]),
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    };
    round_trips(&message);
}
