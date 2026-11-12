use super::*;

#[test]
fn a_block_made_on_an_offline_device_shows_in_its_files_and_reaches_the_other_once_it_reconnects() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    phone.offline();
    start_adding_to_my_files(&mut phone, "Text");
    phone.type_text("Itinerary");
    phone.key("Enter");
    phone.wait("TreeItem \"Itinerary\"");

    phone.online();
    phone.synced();
    laptop.wait("TreeItem \"Itinerary\"");
    phone.close();
    laptop.close();
}
