use super::*;

#[test]
fn the_organizations_screen_takes_an_invite_link() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.wait("#host.join.link");
    host.crop("#host.join");
    host.snapshot("the_organizations_screen_takes_an_invite_link");
    host.close();
}
