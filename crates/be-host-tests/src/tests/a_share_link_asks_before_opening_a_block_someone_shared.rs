use super::*;

#[test]
fn a_share_link_asks_before_opening_a_block_someone_shared() {
    let server = Server::start();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
    let mut host = HostTest::device(&dev);
    host.click("#file-tree.new");
    host.click("#file-tree.new-workspace");
    host.wait("TextInput \"Name\" value=\"New workspace\" focused");
    host.type_text("Team");
    host.key("Enter");
    host.wait("TreeItem \"Team\"");
    host.run(&["click", "TreeItem \"Team\"", "right"]);
    host.click("#file-tree.section.share");
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
    // Made on Dev's device, the link names the server as that device reaches it.
    let link = link.replacen(&dev_device.server_url(), &host.server_url(), 1);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.share-link");
    host.click("#host.share-link.paste");
    host.type_text(&link);
    host.key("Enter");
    host.wait("#host.share-link.offer");
    // The question holds the keys: Escape declines, back to the paste field.
    host.wait("Button \"Open\" focused #host.share-link.offered.accept");
    host.key("Escape");
    host.pause(500);
    host.gone("AlertDialog \"Open this block?\"");
    host.wait("TextInput \"Share link\" focused");
    // Asked again, it takes the keys again: Enter opens.
    host.type_text(&link);
    host.key("Enter");
    host.wait("Button \"Open\" focused #host.share-link.offered.accept");
    host.crop("AlertDialog \"Open this block?\"");
    host.snapshot("a_share_link_asks_before_opening_a_block_someone_shared");
    host.key("Enter");
    host.wait("#host.share-link.opened");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Open a share link\"");
    host.wait("TreeItem \"Team\"");
    host.close();
    dev_device.close();
}
