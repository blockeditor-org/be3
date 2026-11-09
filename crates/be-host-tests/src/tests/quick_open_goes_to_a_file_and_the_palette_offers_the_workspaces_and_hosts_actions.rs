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
    host.click("#text.surface");
    host.key("ctrl+shift+p");
    host.wait("#host.palette.search");
    host.type_text("go to a file");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    let quick_open = rows
        .iter()
        .find(|key| key.ends_with("/workspace.quick-open"))
        .unwrap_or_else(|| panic!("the workspace offers quick open from inside the text: {rows:?}"))
        .clone();
    host.click(&format!("{ROW}{quick_open}"));
    host.run(&["settle"]);
    host.wait("#host.palette.search");
    host.type_text("text");
    host.run(&["settle"]);
    let rows = listed(&mut host);
    let tree = host.tree();
    assert!(
        rows.first().is_some_and(|key| !key.contains('/')),
        "quick open lists blocks, the text first: {rows:?}\n{tree}"
    );
    host.snapshot("quick_open_lists_the_blocks_the_host_has_loaded");
    host.key("Enter");
    host.gone("#host.palette.search");
    host.wait("Tab \"Text\" selected");

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
