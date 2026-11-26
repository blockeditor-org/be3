use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_asks_before_connecting_two_accounts_by_a_contact_link() {
    // Two devices with the root in place of be-host's UI: one makes a contact link, the other
    // opens it, asks before connecting, and lists the contact once connected, as be-host's do
    // (a_contact_link_asks_before_connecting_two_accounts).
    let server = server();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut dev_device = HostTest::root_device(&dev, ROOT);
    dev_device.run(&["settle"]);
    let link = make_contact_link(&mut dev_device);
    let mut host = HostTest::root_device(&ada, ROOT);
    host.run(&["settle"]);
    open_contact_link(&mut host, &dev_device, &link);
    host.crop("AlertDialog \"Connect?\"");
    host.snapshot("be_root_asks_before_connecting_two_accounts_by_a_contact_link");
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.crop("Dialog \"Contacts\"");
    host.snapshot("be_root_asks_before_connecting_two_accounts_and_lists_the_contact");
    host.close();
    dev_device.close();
}
