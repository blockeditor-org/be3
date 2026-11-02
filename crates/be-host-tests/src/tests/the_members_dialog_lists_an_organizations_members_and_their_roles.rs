use super::*;

#[test]
fn the_members_dialog_lists_an_organizations_members_and_their_roles() {
    let mut app = AppTest::start();
    app.click("#file-tree.app-menu");
    app.click("#app.menu.switch-organization");
    app.click("TextInput \"Organization name\"");
    app.type_text("Team");
    app.click("Button \"Create\"");
    app.wait("#file-tree.app-menu");
    app.click("#file-tree.app-menu");
    app.click("#app.menu.members");
    app.wait("Dialog \"Members\"");
    app.wait("Label value=\"Administrator\"");
    app.crop("Dialog \"Members\"");
    app.snapshot("the_members_dialog_lists_an_organizations_members_and_their_roles");
    app.close();
}
