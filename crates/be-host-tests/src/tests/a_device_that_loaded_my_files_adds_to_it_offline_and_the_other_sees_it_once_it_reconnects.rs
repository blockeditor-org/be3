use super::*;

/// Adding a block to My files edits My files' content, so a device that has loaded it keeps that
/// content and can add to it offline.
#[test]
fn a_device_that_loaded_my_files_adds_to_it_offline_and_the_other_sees_it_once_it_reconnects() {
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
    start_adding_to_my_files(&mut phone, "Text");
    phone.type_text("Itinerary");
    phone.key("Enter");
    phone.wait("TreeItem \"Itinerary\"");
    phone.wait("TreeItem \"Recipes\"");

    phone.online();
    phone.synced();
    laptop.wait("TreeItem \"Itinerary\"");
    laptop.wait("TreeItem \"Recipes\"");
    phone.close();
    laptop.close();
}
