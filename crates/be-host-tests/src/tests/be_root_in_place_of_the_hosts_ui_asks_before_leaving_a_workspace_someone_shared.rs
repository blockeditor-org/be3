use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_asks_before_leaving_a_workspace_someone_shared() {
    // Ada shares a workspace with Dev, whose device runs the root in place of be-host's UI:
    // leaving it from the files asks first, in the root's dialog, as be-host's does
    // (leaving_a_workspace_someone_shared_asks_first).
    let server = server();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut dev_device = HostTest::root_device(&dev, ROOT);
    dev_device.run(&["settle"]);
    let link = make_contact_link(&mut dev_device);
    let mut host = HostTest::device(&ada);
    open_contact_link(&mut host, &dev_device, &link);
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
    host.snapshot("be_root_asks_before_leaving_a_workspace_someone_shared");
    host.click("Button \"Stay\"");
    host.gone("AlertDialog \"Leave \u{201c}Team\u{201d}?\"");
    host.close();
}
