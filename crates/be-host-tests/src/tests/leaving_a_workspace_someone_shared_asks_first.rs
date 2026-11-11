use super::*;

#[test]
fn leaving_a_workspace_someone_shared_asks_first() {
    let mut host = HostTest::start();
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

    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    host.click("#account.local");
    host.click("#account.name");
    host.type_text("Ada");
    host.click("#account.confirm");
    host.click("Button \"Personal\"");
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
    host.click("Button \"Dev\"");
    host.click("#share.add");
    host.wait("Label value=\"Dev\"");
    host.click("#share.close");

    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("#account.open.Dev");
    host.gone("#account.open.Dev");
    host.wait("TreeItem \"Team\"");
    host.run(&["click", "TreeItem \"Team\"", "right"]);
    host.click("#file-tree.section.leave");
    host.wait("AlertDialog \"Leave?\"");
    host.crop("AlertDialog \"Leave?\"");
    host.snapshot("leaving_a_workspace_someone_shared_asks_first");
    host.click("Button \"Stay\"");
    host.gone("AlertDialog \"Leave?\"");
    host.close();
}
