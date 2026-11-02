use crate::blocks::{OpenRequest, ShowRequest};

fn shown(block: u8) -> ShowRequest {
    ShowRequest::Block(OpenRequest {
        block_id: [block; 16],
        block_type: [0; 16],
        via: None,
        editor: None,
    })
}

#[test]
fn a_show_request_already_waiting_moves_to_the_end() {
    let mut waiting = Vec::new();
    for request in [shown(1), shown(2), shown(1), shown(1)] {
        ShowRequest::queue(&mut waiting, request);
    }
    assert_eq!(waiting, [shown(2), shown(1)]);
}
