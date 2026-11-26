use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_a_share_link_for_an_outsider_as_a_guest() {
    // A share link that admits anyone with an account, opened on a device with the root in
    // place of be-host's UI by someone outside its organization: the root says they join it as
    // a guest, and the host opens it, as be-host does (an_outsider_opens_a_share_link_as_a_guest).
    let server = server();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut host = HostTest::device(&dev);
    create_organization(&mut host, "Studio");
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
    host.run(&["click", "TreeItem \"Showcase\"", "right"]);
    host.click("#file-tree.section.share");
    host.wait("ComboBox \"Who can open it\"");
    host.click("ComboBox \"Who can open it\"");
    host.click("ListBoxOption \"Anyone with an account here\"");
    host.click("#share.links.make");
    host.wait("#share.links.url");
    let link = share_link(&mut host);
    host.click("#share.close");
    host.synced();

    let dev_device = host;
    let mut host = HostTest::root_device(&ada, ROOT);
    host.run(&["settle"]);
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
    host.snapshot("be_root_opens_a_share_link_for_an_outsider_as_a_guest");
    host.key("Enter");
    host.wait("#host.share-link.opened");
    host.key("Escape");
    host.pause(500);
    // The host opens Studio, where a guest's files pane is what is shared with it.
    host.wait("TreeItem \"Shared with me\"");
    host.wait("TreeItem \"Showcase\"");
    let tree = host.tree();
    for section in ["My files", "Unfiled", "Recently Deleted", "System"] {
        assert!(
            !tree.contains(&format!("TreeItem \"{section}\"")),
            "a guest's files pane has no {section}: {tree}"
        );
    }
    host.gone("#file-tree.new");
    host.synced();
    // be-host names its dock's panes; under the root the workspace's own dock names none, so
    // the files' own area is what is cropped.
    host.crop("#workspace.files");
    host.snapshot("be_root_shows_a_guest_only_what_is_shared_with_it");
    host.close();
    dev_device.close();
}
