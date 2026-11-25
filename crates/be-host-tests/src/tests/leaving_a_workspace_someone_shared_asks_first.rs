use super::*;

#[test]
fn leaving_a_workspace_someone_shared_asks_first() {
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
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Contacts\"");

    host.click("#file-tree.new");
    host.click("#file-tree.new-workspace");
    host.wait("TextInput \"Name\" value=\"New workspace\" focused");
    host.type_text("Team");
    host.key("Enter");
    host.wait("TreeItem \"Team\"");
    host.run(&["click", "TreeItem \"Team\"", "right"]);
    host.click("#file-tree.section.share");
    host.click("#share.query");
    host.type_text("dev");
    host.click("Button \"Dev (dev@example.com)\"");
    host.click("#share.add");
    host.wait("Label value=\"Dev\"");
    host.click("#share.close");

    host.synced();
    host.close();
    let mut host = dev_device;
    host.wait("TreeItem \"Team\"");
    host.run(&["click", "TreeItem \"Team\"", "right"]);
    host.click("#file-tree.section.leave");
    host.wait("AlertDialog \"Leave \u{201c}Team\u{201d}?\"");
    host.crop("AlertDialog \"Leave \u{201c}Team\u{201d}?\"");
    host.snapshot("leaving_a_workspace_someone_shared_asks_first");
    host.click("Button \"Stay\"");
    host.gone("AlertDialog \"Leave \u{201c}Team\u{201d}?\"");
    host.close();
}
