/// The server refuses an acceptance of ownership that would take the account over its bound
/// on blocks (`MAX_BLOCKS_PER_ACCOUNT`) as `OverQuota`: the host answers that in its own words,
/// that the account holds too many blocks to take this one, rather than the server's message.
#[test]
fn an_acceptance_over_the_accounts_bound_says_so_in_the_hosts_words() {
    let refused = be_block_client::ClientError::Refused(
        be_block_protocol::ErrorCode::OverQuota,
        "blocks one account makes in an organization: at most 250000".to_owned(),
    );
    assert_eq!(
        crate::worker::acceptance_failed(&refused),
        crate::worker::TOO_MANY_TO_ACCEPT
    );
    let denied = be_block_client::ClientError::Refused(
        be_block_protocol::ErrorCode::PermissionDenied,
        "only whom it offers it to accepts it".to_owned(),
    );
    assert_ne!(
        crate::worker::acceptance_failed(&denied),
        crate::worker::TOO_MANY_TO_ACCEPT,
        "any other refusal says what it is"
    );
}
