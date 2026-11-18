use super::*;

#[test]
fn an_outsider_opens_a_share_link_as_a_guest() {
    let server = Server::start();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut host = HostTest::device(&dev);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.click("TextInput \"Organization name\"");
    host.type_text("Studio");
    host.click("Button \"Create\"");
    host.wait("#file-tree.host-menu");
    // The organization lets links admit anyone with an account.
    host.click("Tab \"Settings\"");
    host.click("#settings.organization");
    host.click("#settings.organization.share-links");
    host.click("ListBoxOption \"Anyone with an account\"");
    host.synced();
    host.click("Tab \"Files\"");
    host.click("#file-tree.new");
    host.click("#file-tree.new-workspace");
    host.wait("TextInput \"Name\" value=\"New workspace\" focused");
    host.type_text("Showcase");
    host.key("Enter");
    host.wait("TreeItem \"Showcase\"");
    // The dialog follows the policy as this member reads it: it offers the choice once it holds.
    host.run(&["click", "TreeItem \"Showcase\"", "right"]);
    host.click("#file-tree.section.share");
    host.wait("ComboBox \"Who can open it\"");
    host.click("ComboBox \"Who can open it\"");
    host.click("ListBoxOption \"Anyone with an account here\"");
    host.click("#share.links.make");
    host.wait("#share.links.url");
    let link = host
        .tree()
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"")?;
            let value = value.split('"').next()?;
            value.contains("#share/").then(|| value.to_owned())
        })
        .expect("the share dialog shows the link it made");
    host.click("#share.close");
    host.synced();

    let dev_device = host;
    let mut host = HostTest::device(&ada);
    let link = link.replacen(&dev_device.server_url(), &host.server_url(), 1);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.share-link");
    host.click("#host.share-link.paste");
    host.type_text(&link);
    host.key("Enter");
    host.wait("#host.share-link.offer");
    assert!(
        host.tree().contains("You will join Studio as a guest"),
        "an outsider is told they join the organization as a guest"
    );
    host.crop("AlertDialog \"Open this block?\"");
    host.snapshot("an_outsider_opens_a_share_link_as_a_guest");
    host.key("Enter");
    host.wait("#host.share-link.opened");
    host.key("Escape");
    host.pause(500);
    // The host opens Studio, where the block is shared with this guest.
    host.wait("TreeItem \"Showcase\"");
    host.close();
    dev_device.close();
}
