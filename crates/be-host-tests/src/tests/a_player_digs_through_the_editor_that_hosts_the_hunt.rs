use super::*;

/// A hosted block played end to end (guides/blocks/hosted.md): Dev's laptop has a gem hunt
/// open, so its host component runs and hosts it; Ada, given a Join link, opens it in be-host
/// as a Join member, sees only what the host tells her, and digs by asking it, in a cell Dev's
/// editor shows holds a gem. The host writes her dig to the block, which Dev's editor shows,
/// and tells her what she dug up.
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
    // Named by Dev, which only those who may read the block can see.
    start_adding_to_my_files(&mut laptop, "Gem Hunt");
    laptop.type_text("Treasure");
    laptop.key("Enter");
    laptop.wait("TreeItem \"Treasure\"");
    laptop.wait("\"0 of 6 gems found.\"");
    hosts(&mut laptop, true);

    // A Join link, the level a hosted block's links start at.
    laptop.run(&["click", "TreeItem \"Treasure\"", "right"]);
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
    phone.gone("#host.share-link.opened");
    // A Join member holds no key to the block's name, so it is listed by its type.
    phone.click("TreeItem \"Gem Hunt\"");
    phone.gone("\"Treasure\"");
    phone.wait("\"0 of 6 gems found. You found 0.\"");
    // Only the host may read the world: Ada's editor has no world of its own to show.
    phone.gone("#hunt.summary");
    phone.synced();
    phone.snapshot("a_player_digs_through_the_editor_that_hosts_the_hunt");

    // Dev's editor shows where every gem lies; Ada digs one of them. The host writes the dig
    // to the block, where Dev's editor reads it, and tells her what she dug up.
    let gem = gem_cell(&laptop.tree());
    let dig = format!("Button \"Dig row {}, column {}\"", gem / 6 + 1, gem % 6 + 1);
    phone.click(&dig);
    laptop.wait("\"1 of 6 gems found.\"");
    phone.wait("\"1 of 6 gems found. You found 1.\"");
    phone.gone(&dig);

    phone.close();
    laptop.close();
}

/// A cell of the hunt Dev's editor shows holds a gem, still buried.
fn gem_cell(tree: &str) -> u16 {
    tree.lines()
        .find_map(|line| {
            let (_, row) = line.split_once("Button \"Row ")?;
            let (row, rest) = row.split_once(", column ")?;
            let (column, said) = rest.split_once(':')?;
            let (row, column) = (row.parse::<u16>().ok()?, column.parse::<u16>().ok()?);
            said.starts_with(" a gem lies here")
                .then_some((row - 1) * 6 + column - 1)
        })
        .unwrap_or_else(|| panic!("the host's editor shows no gem: {tree}"))
}
