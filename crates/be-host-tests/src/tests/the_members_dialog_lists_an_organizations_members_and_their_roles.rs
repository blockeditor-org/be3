use super::*;

#[test]
fn the_members_dialog_lists_an_organizations_members_and_their_roles() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.switch-organization");
    host.click("TextInput \"Organization name\"");
    host.type_text("Team");
    host.click("Button \"Create\"");
    host.wait("#file-tree.host-menu");
    host.click("#file-tree.host-menu");
    host.click("#host.menu.members");
    host.wait("Dialog \"Members\"");
    host.wait("Label value=\"Administrator\"");
    host.crop("Dialog \"Members\"");
    host.snapshot("the_members_dialog_lists_an_organizations_members_and_their_roles");
    host.close();
}
