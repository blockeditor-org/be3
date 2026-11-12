use super::*;

/// A device lists My files offline from the contents it cached, which hold the head from before
/// another device's commit: here the laptop's, which owns My files' session and sealed the phone's
/// new block into it. The block is safe on the server; the phone shows it again once online.
#[test]
#[ignore = "a device restarted offline lists a container from contents cached before another device's commit"]
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
