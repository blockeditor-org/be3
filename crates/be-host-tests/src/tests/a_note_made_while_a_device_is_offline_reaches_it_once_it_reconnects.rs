use super::*;

#[test]
fn a_note_made_while_a_device_is_offline_reaches_it_once_it_reconnects() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    phone.offline();
    start_adding_to_my_files(&mut laptop, "Text");
    laptop.type_text("Packing list");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Packing list\"");
    laptop.synced();
    assert!(
        !phone.shown("Packing list"),
        "nothing reaches a device while it is offline"
    );

    phone.online();
    phone.synced();
    phone.click("#file-tree.filter");
    phone.type_text("Packing");
    phone.wait("TreeItem \"Packing list\"");
    // The search index's hit, which its own answer brings, once its block's name is known.
    phone.wait("#file-tree.content-hits");
    phone.wait("Button \"Packing list\"");
    phone.run(&["leave"]);
    phone.crop("#workspace.files");
    phone.snapshot("a_note_made_while_a_device_is_offline_reaches_it_once_it_reconnects");
    phone.close();
    laptop.close();
}
