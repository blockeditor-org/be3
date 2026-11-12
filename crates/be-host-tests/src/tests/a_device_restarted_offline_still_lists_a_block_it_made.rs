use super::*;

/// The phone's files listed My files before the phone's seal of its new block reached the server,
/// so the listing it kept is older than My files' summary: offline, it lists what that summary
/// references.
#[test]
fn a_device_restarted_offline_still_lists_a_block_it_made() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    start_adding_to_my_files(&mut phone, "Text");
    phone.type_text("Recipes");
    phone.key("Enter");
    phone.wait("TreeItem \"Recipes\"");
    phone.synced();
    phone.offline();
    phone.restart();
    phone.wait("TreeItem \"Recipes\"");
    phone.close();
    laptop.close();
}
