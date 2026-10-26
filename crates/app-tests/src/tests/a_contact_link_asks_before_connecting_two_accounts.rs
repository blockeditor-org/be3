use super::*;

#[test]
fn a_contact_link_asks_before_connecting_two_accounts() {
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
    app.wait("#app.contacts.offer");
    app.crop("AlertDialog \"Connect?\"");
    app.snapshot("a_contact_link_asks_before_connecting_two_accounts");
    app.click("#app.contacts.offered.accept");
    app.wait("Label value=\"Dev\"");
    app.crop("Dialog \"Contacts\"");
    app.snapshot("a_contact_link_asks_before_connecting_two_accounts_and_lists_the_contact");
    app.close();
}

pub(super) fn confirm_phrase(app: &mut AppTest) {
    let tree = app.tree();
    let phrase = tree
        .lines()
        .find_map(|line| {
            let (_, value) = line.split_once("value=\"1. ")?;
            Some(format!(
                "1. {}",
                value.split('"').next()?.replace("\\n", " ")
            ))
        })
        .expect("the recovery phrase is shown");
    let words: Vec<&str> = phrase
        .split_whitespace()
        .filter(|word| !word.ends_with('.'))
        .collect();
    let asked: Vec<usize> = tree
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("TextInput \"Word ")?;
            rest.split('"').next()?.parse().ok()
        })
        .collect();
    for position in asked {
        app.click(&format!("TextInput \"Word {position}\""));
        app.type_text(words[position - 1]);
    }
    app.click("Button \"I saved them\"");
}
