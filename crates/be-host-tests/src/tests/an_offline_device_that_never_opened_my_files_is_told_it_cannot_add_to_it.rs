use super::*;

/// A block goes into My files by changing My files' content, which a paired device that has only
/// listed it does not hold: offline, it says so instead of making a block that never shows.
#[test]
fn an_offline_device_that_never_opened_my_files_is_told_it_cannot_add_to_it() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    phone.offline();
    open_add_to_my_files(&mut phone);
    choose_type(&mut phone, "Text");
    phone.wait("hasn't been opened on this device");
    assert!(
        !phone.shown("Pane \"Text\""),
        "a refused block opens no tab"
    );
    phone.run(&["leave"]);
    phone.snapshot("an_offline_device_that_never_opened_my_files_is_told_it_cannot_add_to_it");
    phone.close();
    laptop.close();
}
