use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_keeps_the_key_it_saw_of_a_contact_the_server_rolls_back() {
    // Once two accounts are contacts, the server serves one a record of the other that does not
    // open; the root's contacts say the device keeps the record it pinned, as be-host's do
    // (a_contact_the_server_rolls_back_keeps_the_key_this_device_saw).
    let server = server();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut dev_device = HostTest::device(&dev);
    let link = make_contact_link(&mut dev_device);
    let mut host = HostTest::root_device(&ada, ROOT);
    host.run(&["settle"]);
    open_contact_link(&mut host, &dev_device, &link);
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Contacts\"");

    server.spoil_contacts(&ada);
    host.offline();
    host.online();
    host.synced();
    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.wait("Label value=\"Dev\"");
    host.wait("The server's copy of this contact can't be read here");
    host.crop("Dialog \"Contacts\"");
    host.snapshot("be_root_keeps_the_key_it_saw_of_a_contact_the_server_rolls_back");
    host.close();
    dev_device.close();
}
