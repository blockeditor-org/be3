use super::*;

#[test]
fn a_device_restarted_offline_still_lists_a_block_another_device_made() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    start_adding_to_my_files(&mut laptop, "Text");
    laptop.type_text("Garden");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Garden\"");
    laptop.synced();
    phone.wait("TreeItem \"Garden\"");
    phone.synced();
    phone.offline();
    phone.restart();
    phone.wait("TreeItem \"Garden\"");
    phone.close();
    laptop.close();
}
