use super::*;

#[test]
fn a_web_browser_tab_says_why_it_shows_no_page() {
    // be-host shows no web pages yet: a new Web Browser Tab's page says so in its place, below
    // its address bar, which joins the workspace's dock as a band of its natural height.
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Web Browser Tab");
    host.run(&["settle"]);
    host.snapshot("a_web_browser_tab_says_why_it_shows_no_page");
    host.close();
}
