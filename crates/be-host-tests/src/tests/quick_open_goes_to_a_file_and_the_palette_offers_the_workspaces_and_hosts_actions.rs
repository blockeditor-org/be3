use super::*;

const ROW: &str = "#host.palette.item.";

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
fn quick_open_goes_to_a_file_and_the_palette_offers_the_workspaces_and_hosts_actions() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    add_to_my_files(&mut host, "Text");
    host.click("#dock.tab.2.close");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Text\""),
        1,
        "one of the two texts is closed"
    );

    // The workspace's own focus: its tab menu's button, once the menu closes.
    host.click("#dock.editors.menu");
    host.key("Escape");
    host.key("ctrl+p");
    host.wait("#host.palette.search");
    host.type_text("text");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    assert!(
        rows.len() >= 2 && rows[..2].iter().all(|key| !key.contains('/')),
        "quick open lists both texts, the open one first: {rows:?}"
    );
    host.snapshot("quick_open_lists_the_blocks_the_host_has_loaded");
    host.key("Down");
    host.key("Enter");
    host.gone("#host.palette.search");
    host.run(&["settle"]);
    assert_eq!(
        host.count("Tab \"Text\""),
        2,
        "the text that was not open opens in a tab"
    );

    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.wait("#host.palette.search");
    host.type_text("settings");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    assert!(
        rows.iter()
            .any(|key| key.starts_with("be3.workspace-ui/") && key.ends_with("/workspace.settings")),
        "the workspace's Settings is listed from its tab: {rows:?}"
    );
    host.key("ctrl+a");
    host.type_text("theme");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    assert!(
        rows.iter().any(|key| key == "host/host.theme"),
        "and the host's theme, after every app's: {rows:?}"
    );
    host.click(&format!("{ROW}host/host.theme"));
    host.gone("#host.palette.search");
    host.run(&["settle"]);
    host.snapshot("the_theme_action_turns_the_host_light");
    host.close();
}
