use super::*;

/// A hosted block played end to end (guides/blocks/hosted.md): Dev's laptop has a gem hunt
/// open, so its host component runs and hosts the world; Ada, given a Join link, opens it in
/// be-host as a Join member, sees only what the host tells her, and digs by asking it. The
/// host writes her dig to the block, which Dev's editor shows, and tells her what she dug up.
#[test]
fn a_player_digs_through_the_editor_that_hosts_the_hunt() {
    let server = Server::start();
    let dev = server.account("Dev");
    let ada = server.account("Ada");
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
    laptop.click("Button \"Close tab\"");
    laptop.gone("Button \"Close tab\"");
    laptop.click("Tab \"Files\"");
    add_to_my_files(&mut laptop, "Gem Hunt");
    laptop.wait("\"0 of 6 gems found.\"");
    hosts(&mut laptop, true);

    // A Join link, the level a hosted block's links start at.
    laptop.run(&["click", "TreeItem \"Gem Hunt\"", "right"]);
    laptop.click("MenuItem \"Share\"");
    laptop.wait("ComboBox \"Who can open it\"");
    laptop.click("ComboBox \"Who can open it\"");
    laptop.click("ListBoxOption \"Anyone with an account here\"");
    laptop.wait("\"Join the hunt\"");
    laptop.click("#share.links.make");
    laptop.wait("#share.links.url");
    let link = share_link(&mut laptop);
    laptop.click("#share.close");
    laptop.synced();

    // Ada opens the link and the hunt: she joins, so her editor shows what the host tells it.
    let mut phone = HostTest::device(&ada);
    let link = link.replacen(&laptop.server_url(), &phone.server_url(), 1);
    phone.click("#file-tree.host-menu");
    phone.click("#host.menu.share-link");
    phone.click("#host.share-link.paste");
    phone.type_text(&link);
    phone.key("Enter");
    phone.wait("#host.share-link.offer");
    phone.key("Enter");
    phone.wait("#host.share-link.opened");
    phone.key("Escape");
    phone.pause(500);
    phone.gone("\"Nothing has been shared with you\"");
    phone.click("TreeItem \"Gem Hunt\"");
    phone.wait("\"0 of 6 gems found. You found 0.\"");
    // Only the host may read the world: Ada's editor has no world of its own to show.
    phone.gone("#hunt.summary");
    phone.synced();
    phone.snapshot("a_player_digs_through_the_editor_that_hosts_the_hunt");

    // She digs the top left cell. The host writes the dig to the block, where Dev's editor
    // reads it, and tells her what she dug up.
    phone.click("Button \"Dig row 1, column 1\"");
    phone.gone("Button \"Dig row 1, column 1\"");
    laptop.gone("Button \"Dig row 1, column 1\"");
    // The cell held a gem or it did not; both sides count the same.
    let found = match phone.tree().contains("\"1 of 6 gems found. You found 1.\"") {
        true => 1,
        false => 0,
    };
    phone.wait(&format!("\"{found} of 6 gems found. You found {found}.\""));
    laptop.wait(&format!("\"{found} of 6 gems found.\""));

    phone.close();
    laptop.close();
}
