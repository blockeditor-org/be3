use super::a_contact_link_asks_before_connecting_two_accounts::confirm_phrase;
use super::*;

#[test]
fn leaving_a_workspace_someone_shared_asks_first() {
    let mut app = AppTest::start();
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.contacts");
    app.click("#app.contacts.new-link");
    app.wait("#app.contacts.link");
    let link = app
        .tree()
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"")?;
            let value = value.split('"').next()?;
            value.contains("#contact/").then(|| value.to_owned())
        })
        .expect("the dialog shows the link it made");
    app.key("Escape");
    app.pause(500);
    app.gone("Dialog \"Contacts\"");

    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.accounts");
    app.click("Button \"Add account\"");
    app.click("#account.local");
    app.click("#account.name");
    app.type_text("Ada");
    app.click("#account.confirm");
    app.wait("Label value=\"Check that you saved them\"");
    confirm_phrase(&mut app);
    app.click("Button \"Personal\"");
    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.contacts");
    app.click("#app.contacts.paste");
    app.type_text(&link);
    app.click("#app.contacts.open");
    app.click("#app.contacts.offered.accept");
    app.wait("Label value=\"Dev\"");
    app.key("Escape");
    app.pause(500);
    app.gone("Dialog \"Contacts\"");

    app.click("#file-tree.new-workspace");
    app.wait("TextInput \"Name\"");
    app.type_text("Team");
    app.key("Enter");
    app.wait("TreeItem \"Team\"");
    app.run(&["click", "TreeItem \"Team\"", "right"]);
    app.click("#file-tree.section.share");
    app.click("#share.query");
    app.type_text("dev");
    app.click("Button \"Dev\"");
    app.click("#share.add");
    app.wait("Label value=\"Dev\"");
    app.click("#share.close");

    app.click("#file-tree.desktop-app-menu");
    app.click("#app.menu.accounts");
    app.wait("Label value=\"Dev\"");
    let tree = app.tree();
    let open = tree
        .lines()
        .skip_while(|line| !line.contains("Label value=\"Dev\""))
        .find(|line| line.contains("Button \"Open\""))
        .and_then(|line| line.split(" at ").nth(1))
        .and_then(|place| place.split(' ').next())
        .expect("the accounts screen offers to open Dev's account")
        .to_owned();
    let (x, y) = open.split_once(',').expect("a place is X,Y");
    let x: f32 = x.parse().unwrap();
    let y: f32 = y.parse().unwrap();
    app.run(&["click", &format!("{},{}", x + 10.0, y + 10.0)]);
    app.wait("TreeItem \"Team\"");
    app.run(&["click", "TreeItem \"Team\"", "right"]);
    app.click("#file-tree.section.leave");
    app.wait("AlertDialog \"Leave?\"");
    app.crop("AlertDialog \"Leave?\"");
    app.snapshot("leaving_a_workspace_someone_shared_asks_first");
    app.click("Button \"Stay\"");
    app.gone("AlertDialog \"Leave?\"");
    app.close();
}
