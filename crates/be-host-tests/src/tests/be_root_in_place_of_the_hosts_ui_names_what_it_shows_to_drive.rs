use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_names_what_it_shows_to_drive() {
    // With the root in place of be-host's UI, drive reads the root's accessibility tree, as
    // it reads be-host's own: its own surfaces' (its menu button) and those of the apps it
    // embeds (the files' Account and settings button), placed where they show, so a press by
    // name lands where be-host's would.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    let files = host.place("#file-tree.host-menu");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    assert_eq!(
        host.place("#file-tree.host-menu"),
        files,
        "the files' button is where be-host's own UI shows it"
    );
    host.wait("Button \"Menu\"");
    host.click("#file-tree.host-menu");
    host.run(&["settle"]);
    host.snapshot("be_root_draws_the_hosts_menu_panel");
    host.close();
}
