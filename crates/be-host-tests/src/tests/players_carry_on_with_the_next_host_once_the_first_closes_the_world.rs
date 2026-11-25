use super::*;

/// A hosted block's players carry on with the next host once the first closes the world
/// (guides/blocks/hosted.md, "Hosting"): two devices of one account have the counter open, the
/// first hosting it, and two players reach it as raw peers through a Join link, each answered
/// by the host as it handshakes. Once the first device closes its editor its lease
/// goes, the second device's component claims it, and both players hear the new host and are
/// answered by it.
#[test]
fn players_carry_on_with_the_next_host_once_the_first_closes_the_world() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);

    // An organization whose links admit anyone with an account, as a game's would.
    laptop.click("#file-tree.host-menu");
    laptop.click("#host.menu.switch-organization");
    laptop.click("TextInput \"Organization name\"");
    laptop.type_text("Studio");
    laptop.click("Button \"Create\"");
    laptop.wait("#file-tree.host-menu");
    laptop.click("Tab \"Settings\"");
    laptop.click("#settings.organization");
    laptop.click("#settings.organization.share-links");
    laptop.click("ListBoxOption \"Anyone with an account\"");
    laptop.synced();
    // The organization's settings opened in a tab of their own, which goes.
    laptop.click("Button \"Close tab\"");
    laptop.gone("Button \"Close tab\"");
    laptop.click("Tab \"Files\"");
    // Paired once the laptop is in the organization, the phone opens it too.
    let mut phone = laptop.another_device();
    phone.click("#file-tree.host-menu");
    phone.click("#host.menu.switch-organization");
    phone.click("Button \"Studio\"");
    phone.wait("#file-tree.host-menu");
    add_to_my_files(&mut laptop, "Counter");
    hosts(&mut laptop, true);

    // A Join link, the level a hosted block's links start at.
    laptop.run(&["click", "TreeItem \"Counter\"", "right"]);
    laptop.click("MenuItem \"Share\"");
    laptop.wait("ComboBox \"Who can open it\"");
    laptop.click("ComboBox \"Who can open it\"");
    laptop.click("ListBoxOption \"Anyone with an account here\"");
    laptop.click("#share.links.make");
    laptop.wait("#share.links.url");
    let link = laptop
        .tree()
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"")?;
            let value = value.split('"').next()?;
            value.contains("#share/").then(|| value.to_owned())
        })
        .expect("the share dialog shows the link it made");
    laptop.click("#share.close");
    laptop.synced();

    // The phone opens the counter too, and stands by while the laptop hosts.
    phone.wait("TreeItem \"Counter\"");
    phone.click("TreeItem \"Counter\"");
    phone.wait("#counter.value");
    hosts(&mut phone, false);

    let mut players = [server.player("Ada"), server.player("Bo")];
    let mut first = Vec::new();
    for player in &mut players {
        let block = player.open_share_link(&link);
        let host = player
            .join(block)
            .expect("the laptop hosts the counter as the player joins");
        player.handshake(block, host, b"hello");
        first.push((block, host));
    }

    // The laptop closes the world; the phone hosts on, and each player is answered by it.
    laptop.click("Button \"Close tab\"");
    laptop.gone("#counter.value");
    hosts(&mut phone, true);
    for (player, (block, host)) in players.iter_mut().zip(first) {
        let next = player.host_other_than(block, Some(host));
        player.handshake(block, next, b"still here");
    }

    drop(players);
    phone.close();
    laptop.close();
}
