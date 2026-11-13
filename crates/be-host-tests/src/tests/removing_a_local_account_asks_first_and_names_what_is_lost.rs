use super::*;

#[test]
fn removing_a_local_account_asks_first_and_names_what_is_lost() {
    let mut host = HostTest::start(Options::default());
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("#account.options.Dev");
    host.click("MenuItem \"Remove account…\"");
    host.wait("AlertDialog \"Remove Dev?\"");
    host.crop("AlertDialog \"Remove Dev?\"");
    host.snapshot("removing_a_local_account_asks_first_and_names_what_is_lost");
    host.click("Button \"Keep\"");
    host.gone("AlertDialog \"Remove Dev?\"");
    host.wait("#account.open.Dev");

    host.click("#account.options.Dev");
    host.click("MenuItem \"Remove account…\"");
    host.click("#host.remove-account.remove");
    host.gone("#account.open.Dev");
    assert!(host.shown("No accounts yet."));
    host.close();
}
