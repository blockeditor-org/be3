use super::*;

/// Once two accounts are contacts, the server serves one a record of the other that does not
/// open, as a server tampering with it would; the device keeps the record it pinned and says so
/// in the contacts (guides/blocks/keys.md, "Contacts").
#[test]
fn a_contact_the_server_rolls_back_keeps_the_key_this_device_saw() {
    let server = Server::start();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut host = HostTest::device(&dev);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.click("#host.contacts.new-link");
    host.wait("#host.contacts.link");
    let link = host
        .tree()
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"")?;
            let value = value.split('"').next()?;
            value.contains("#contact/").then(|| value.to_owned())
        })
        .expect("the dialog shows the link it made");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Contacts\"");

    let dev_device = host;
    let mut host = HostTest::device(&ada);
    let link = link.replacen(&dev_device.server_url(), &host.server_url(), 1);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.click("#host.contacts.paste");
    host.type_text(&link);
    host.click("#host.contacts.open");
    host.wait("#host.contacts.offer");
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Contacts\"");

    server.spoil_contact(&ada, &dev);
    host.offline();
    host.online();
    host.synced();
    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.wait("Label value=\"Dev\"");
    host.wait("The server's copy of this contact can't be read here");
    host.crop("Dialog \"Contacts\"");
    host.snapshot("a_contact_the_server_rolls_back_keeps_the_key_this_device_saw");
    host.close();
    dev_device.close();
}
