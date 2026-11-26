use super::*;

#[test]
fn a_contact_link_asks_before_connecting_two_accounts() {
    let server = server();
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
    // Made on Dev's device, the link names the server as that device reaches it.
    let link = link.replacen(&dev_device.server_url(), &host.server_url(), 1);

    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.click("#host.contacts.paste");
    host.type_text(&link);
    host.click("#host.contacts.open");
    host.wait("#host.contacts.offer");
    host.crop("AlertDialog \"Connect?\"");
    host.snapshot("a_contact_link_asks_before_connecting_two_accounts");
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.crop("Dialog \"Contacts\"");
    host.snapshot("a_contact_link_asks_before_connecting_two_accounts_and_lists_the_contact");
    host.close();
    dev_device.close();
}
