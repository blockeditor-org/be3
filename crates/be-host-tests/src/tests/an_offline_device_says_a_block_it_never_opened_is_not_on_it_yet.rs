use super::*;

/// A paired device lists another device's block but has never read it, so offline its tab says
/// it is not on this device yet instead of a blank editor, and opens it once back online.
#[test]
fn an_offline_device_says_a_block_it_never_opened_is_not_on_it_yet() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    start_adding_to_my_files(&mut laptop, "Text");
    laptop.type_text("Garden");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Garden\"");
    laptop.click("#text.surface");
    laptop.type_text("Plant the tomatoes");
    laptop.wait("Plant the tomatoes");
    laptop.synced();
    phone.wait("TreeItem \"Garden\"");

    phone.offline();
    phone.click("TreeItem \"Garden\"");
    phone.wait("#workspace.out-of-reach");
    phone.run(&["leave"]);
    phone.snapshot("an_offline_device_says_a_block_it_never_opened_is_not_on_it_yet");

    phone.online();
    phone.synced();
    phone.gone("#workspace.out-of-reach");
    phone.wait("Pane \"Text\"");
    phone.wait("Plant the tomatoes");
    phone.close();
    laptop.close();
}
