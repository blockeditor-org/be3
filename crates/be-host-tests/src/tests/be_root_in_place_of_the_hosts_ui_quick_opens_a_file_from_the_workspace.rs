use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const ROW: &str = "#root.palette.item.";

fn listed(host: &mut HostTest) -> Vec<String> {
    host.tree()
        .lines()
        .filter_map(|line| {
            let at = line.find(ROW)?;
            Some(line[at + ROW.len()..].split_whitespace().next()?.to_owned())
        })
        .collect()
}

#[test]
fn be_root_in_place_of_the_hosts_ui_quick_opens_a_file_from_the_workspace() {
    // be-host's own UI makes two texts and closes the tab of one; started again with the root in
    // place of its UI, the workspace's Ctrl+P opens quick open in the one palette the root draws,
    // listing the blocks the host finds for what is typed (`host.find-blocks`), as
    // `quick_open_goes_to_a_file_and_the_palette_offers_the_workspaces_and_hosts_actions` does
    // under be-host's; choosing the text that is not open opens it in a tab
    // (`host.open-block`).
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Text");
    add_to_my_files(&mut host, "Text");
    host.click("#dock.tab.2.close");
    host.run(&["settle"]);
    host.restart_as_root(ROOT);
    host.wait("Tab \"Text\"");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Text\""),
        1,
        "one of the two texts is open"
    );
    // The workspace's own focus: its tab menu's button, once the menu closes.
    host.click("#dock.editors.menu");
    host.key("Escape");
    host.key("ctrl+p");
    host.wait("#root.palette.search");
    host.type_text("text");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    assert!(rows.len() >= 2, "quick open lists both texts: {rows:?}");
    host.snapshot("be_root_quick_open_lists_the_blocks_the_host_finds");
    // The open text is listed first: choosing it opens no other tab.
    host.key("Enter");
    host.gone("#root.palette.search");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Text\""),
        1,
        "the first row is the text already open"
    );
    host.click("#dock.editors.menu");
    host.key("Escape");
    host.key("ctrl+p");
    host.wait("#root.palette.search");
    host.type_text("text");
    host.run(&["settle"]);
    host.key("Down");
    host.key("Enter");
    host.gone("#root.palette.search");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Text\""),
        2,
        "the text that was not open opens in a tab"
    );
    // The command palette lists the host's own rows after every editor's: its theme, which the
    // host switches.
    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.wait("#root.palette.search");
    host.type_text("theme");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    assert!(
        rows.iter().any(|key| key == "host/host.theme"),
        "the host's theme is listed: {rows:?}"
    );
    host.click(&format!("{ROW}host/host.theme"));
    host.gone("#root.palette.search");
    host.run(&["settle"]);
    host.snapshot("be_root_the_theme_action_turns_the_host_light");
    host.close();
}
