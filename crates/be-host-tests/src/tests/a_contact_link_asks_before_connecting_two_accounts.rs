use super::*;

#[test]
fn a_contact_link_asks_before_connecting_two_accounts() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.click("#host.contacts.new-link");
    host.wait("#host.contacts.link");
    let link = host
        .tree()
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"")?;
            let value = value.split('"').next()?;
            value.contains("#contact/").then(|| value.to_owned())
        })
        .expect("the dialog shows the link it made");
    host.key("Escape");
    host.pause(500);
    host.gone("Dialog \"Contacts\"");

    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    host.click("#account.local");
    host.click("#account.name");
    host.type_text("Ada");
    host.click("#account.confirm");
    host.click("Button \"Personal\"");

    host.click("#file-tree.host-menu");
    host.click("#host.menu.contacts");
    host.click("#host.contacts.paste");
    host.type_text(&link);
    host.click("#host.contacts.open");
    host.wait("#host.contacts.offer");
    host.crop("AlertDialog \"Connect?\"");
    host.snapshot("a_contact_link_asks_before_connecting_two_accounts");
    host.click("#host.contacts.offered.accept");
    host.wait("Label value=\"Dev\"");
    host.crop("Dialog \"Contacts\"");
    host.snapshot("a_contact_link_asks_before_connecting_two_accounts_and_lists_the_contact");
    host.close();
}
