use super::*;

/// Moving a block changes the content of the container it leaves and the one it joins. Offline, a
/// device that never read the folder cannot change it, so the move is refused with a notice and
/// the block stays in My files, on this device and everywhere once it reconnects.
#[test]
fn an_offline_device_refuses_to_move_a_block_into_a_folder_it_never_opened() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    start_adding_to_my_files(&mut laptop, "Folder");
    laptop.type_text("Archive");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Archive\"");
    laptop.synced();
    phone.wait("TreeItem \"Archive\"");
    start_adding_to_my_files(&mut phone, "Text");
    phone.type_text("Recipes");
    phone.key("Enter");
    phone.wait("TreeItem \"Recipes\"");
    phone.synced();

    phone.offline();
    let from = phone.place("TreeItem \"Recipes\"");
    let to = phone.place("TreeItem \"Archive\"");
    let (x, y) = (from.0 + from.2 / 2.0, from.1 + from.3 / 2.0);
    let target = to.1 + to.3 / 2.0;
    phone.run(&["down", &format!("{x},{y}")]);
    for step in 1..=4 {
        let y = y + (target - y) * step as f32 / 4.0;
        phone.run(&["move", &format!("{x},{y}")]);
    }
    phone.run(&["up"]);
    phone.wait("Archive hasn't been opened on this device");
    phone.wait("TreeItem \"Recipes\"");

    phone.online();
    phone.synced();
    laptop.wait("TreeItem \"Recipes\"");
    laptop.click("TreeItem \"Archive\"");
    laptop.wait("#folder.empty");
    phone.close();
    laptop.close();
}
