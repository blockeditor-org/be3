use super::*;

/// A block added from the files' New button while they are zoomed into My files is made in
/// My files, not in Recently Deleted, on a second device of the account that restarted.
#[test]
fn a_block_added_while_zoomed_into_my_files_is_made_in_it() {
    let server = Server::start();
    server.slow(std::time::Duration::from_millis(100));
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    add_to_my_files(&mut laptop, "Checklist");
    laptop.synced();
    let mut host = laptop.another_device();
    host.wait("TreeItem \"Checklist\"");
    host.restart();
    host.click("TreeItem \"My files\"");
    host.click(NEW);
    choose_type(&mut host, "Text");
    host.wait("Pane \"Text\"");
    host.wait("TextInput \"Name\" value=\"Text\" focused");
    host.key("Enter");
    host.gone("TextInput \"Name\" value=\"Text\"");
    host.wait("TreeItem \"Text\"");
    host.click("#file-tree.back");
    host.wait("TreeItem \"Recently Deleted\"");
    let (mine, text, deleted) = (
        host.place("TreeItem \"My files\""),
        host.place("TreeItem \"Text\""),
        host.place("TreeItem \"Recently Deleted\""),
    );
    assert!(
        mine.1 < text.1 && text.1 < deleted.1,
        "the new block is listed in My files, above Recently Deleted"
    );
    assert!(
        !host.shown("This block is in Recently Deleted."),
        "the new block is not deleted"
    );
    host.close();
    laptop.close();
}
