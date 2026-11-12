use super::*;

/// A device whose network drops what it sends, from the moment it starts, opens from what it
/// keeps: My files and the document it had open, which takes edits.
#[test]
fn a_device_that_cannot_reach_its_server_opens_its_files_and_edits_at_once() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();

    phone.unreachable();
    phone.restart();
    phone.wait("TreeItem \"Recipes\"");
    phone.click("#text.surface");
    phone.key("ctrl+End");
    phone.type_text(" for soup");
    phone.wait("Recipes for soup");
    phone.close();
}
