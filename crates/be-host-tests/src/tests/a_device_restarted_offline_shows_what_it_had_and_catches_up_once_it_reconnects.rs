use super::*;

#[test]
fn a_device_restarted_offline_shows_what_it_had_and_catches_up_once_it_reconnects() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    add_to_my_files(&mut laptop, "Text");
    laptop.synced();
    phone.wait("TreeItem \"Text\"");

    phone.offline();
    phone.restart();
    phone.wait("TreeItem \"Text\"");
    start_adding_to_my_files(&mut laptop, "Text");
    laptop.type_text("Shopping");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Shopping\"");
    laptop.synced();
    assert!(
        !phone.shown("Shopping"),
        "a device started offline shows only what it kept"
    );

    phone.online();
    phone.synced();
    phone.wait("TreeItem \"Shopping\"");
    phone.close();
    laptop.close();
}
