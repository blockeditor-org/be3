use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens the organizations from the files' host menu and paints where an invite link is
/// pasted, as `painted`.
fn show_where_an_invite_link_goes(host: &mut HostTest, painted: &str) {
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.wait("#host.join.link");
    host.crop("#host.join");
    host.snapshot(painted);
}

#[test]
fn be_root_in_place_of_the_hosts_ui_takes_an_invite_link_on_its_organizations() {
    // be-host's own organizations take an invite link (`the_organizations_screen_takes_an_invite_link`).
    // Started again with the root in place of its UI, the root's take it the same way.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    show_where_an_invite_link_goes(&mut host, "the_hosts_organizations_take_an_invite_link");
    host.click("Button \"Personal\"");
    host.wait("#file-tree.host-menu");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    show_where_an_invite_link_goes(&mut host, "be_roots_organizations_take_an_invite_link");
    host.close();
}
