use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_says_why_a_web_browser_tab_shows_no_page() {
    // be-host's own UI makes a Web Browser Tab; started again with the root in place of its UI,
    // the tab's web view fails as under be-host, and its page says why in its place.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Web Browser Tab");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.snapshot("be_root_says_why_a_web_browser_tab_shows_no_page");
    host.close();
}
