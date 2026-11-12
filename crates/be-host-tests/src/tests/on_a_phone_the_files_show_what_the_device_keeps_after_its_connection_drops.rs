use super::*;

/// Going back on a phone closes the document and shows My files again, from what the device
/// keeps whether or not it is connected.
#[test]
#[ignore = "a command that needs the server (set parent) holds the block store's loop until the silent connection is declared lost"]
fn on_a_phone_the_files_show_what_the_device_keeps_after_its_connection_drops() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut phone = HostTest::device(&dev);
    add_to_my_files(&mut phone, "Text");
    phone.click("#text.surface");
    phone.type_text("Recipes");
    phone.synced();
    phone.run(&["resize", "390x844"]);
    phone.run(&["settle"]);

    phone.unreachable();
    phone.run(&["--no-settle", "back"]);
    phone.pause(1_000);
    phone.wait("TreeItem \"Recipes\"");
    phone.gone("#editor.loading");

    phone.online();
    phone.synced();
    phone.click("TreeItem \"Recipes\"");
    phone.wait("#text.surface");
    phone.run(&["back"]);
    phone.wait("TreeItem \"Recipes\"");
    phone.gone("#editor.loading");
    phone.close();
}
