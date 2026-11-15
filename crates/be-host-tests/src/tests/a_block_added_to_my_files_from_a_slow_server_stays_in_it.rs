use super::*;

/// A block added to My files while the server answers slowly stays in it. My files' replica
/// opens before it has joined its session and takes the new child meanwhile; a status its module
/// made before it took that edit must not let the host forget it, or joining opens the replica
/// again without it and the new block reads as deleted.
#[test]
fn a_block_added_to_my_files_from_a_slow_server_stays_in_it() {
    let server = Server::start();
    server.slow(std::time::Duration::from_millis(100));
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.synced();
    assert!(
        !phone.shown("This block is in Recently Deleted."),
        "the new block is not deleted"
    );
    phone.close();
}
