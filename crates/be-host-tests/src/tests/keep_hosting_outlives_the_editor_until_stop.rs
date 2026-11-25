use super::*;

const STOP: &str = "MenuItem \"Stop hosting Counter\"";
const HOSTING_NOW: &str = "MenuItem \"Stop hosting Counter\" description=\"Hosting here now\"";

/// The count the open counter shows.
fn count(host: &mut HostTest) -> String {
    host.wait("#counter.value");
    let tree = host.tree();
    let line = tree
        .lines()
        .find(|line| line.contains("#counter.value"))
        .expect("the counter shows its count");
    line.split("value=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no count in {line}"))
        .to_owned()
}

/// Keep hosting, from the frame menu, keeps a block's host component running on this device
/// after its editor closes (guides/blocks/hosted.md, "Hosting"), through a reconnect, until the
/// host menu's Stop. Which device hosts shows in the other's frame menu, which offers Keep
/// hosting only where it hosts; the counter's host component adds one as it starts, so a
/// count that moves shows a component started.
#[test]
fn keep_hosting_outlives_the_editor_until_stop() {
    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();

    add_to_my_files(&mut laptop, "Counter");
    hosts(&mut laptop, true);
    laptop.click(MORE);
    laptop.click(KEEP_HOSTING);
    laptop.click("Button \"Close tab\"");
    laptop.gone("#counter.value");
    laptop.click("#file-tree.host-menu");
    laptop.wait(STOP);
    laptop.key("Escape");

    // The laptop hosts on with no editor open, so the phone's editor stands by.
    phone.wait("TreeItem \"Counter\"");
    phone.click("TreeItem \"Counter\"");
    hosts(&mut phone, false);

    // Offline, the laptop's lease goes and the phone hosts; back online, the laptop keeps it.
    laptop.offline();
    hosts(&mut phone, true);
    laptop.online();
    laptop.synced();
    laptop.click("#file-tree.host-menu");
    laptop.wait(STOP);
    laptop.key("Escape");

    // With the phone's editor closed, the kept laptop hosts again: its component starts, and
    // the count moves.
    let before = count(&mut phone);
    phone.click("Button \"Close tab\"");
    phone.gone("#counter.value");
    // The laptop's host menu says once it hosts the counter now rather than standing by, so
    // the phone opens it only then, rather than racing the laptop to claim it.
    laptop.click("#file-tree.host-menu");
    laptop.wait(HOSTING_NOW);
    laptop.key("Escape");
    phone.click("TreeItem \"Counter\"");
    phone.wait("#counter.value");
    phone.gone(&format!("value=\"{before}\" #counter.value"));
    hosts(&mut phone, false);

    // Stop ends it: the laptop lets the block go, and the phone's editor hosts.
    laptop.click("#file-tree.host-menu");
    laptop.click(STOP);
    hosts(&mut phone, true);
    laptop.click("#file-tree.host-menu");
    laptop.wait("#host.menu.contacts");
    laptop.gone(STOP);
    laptop.key("Escape");

    phone.close();
    laptop.close();
}
