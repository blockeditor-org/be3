use be_block_client::ClientError;
use be_block_protocol::ErrorCode;

use crate::search::not_there_yet;

#[test]
fn a_search_index_made_before_the_server_has_its_root_is_made_again_without_an_error() {
    let refused = |code| ClientError::Refused(code, "that block".into());
    assert!(
        not_there_yet(&refused(ErrorCode::BlockNotFound)),
        "the root's own create has not reached the server"
    );
    for code in [ErrorCode::InvalidRequest, ErrorCode::PermissionDenied] {
        assert!(!not_there_yet(&refused(code)), "{code:?} is a real refusal");
    }
}
