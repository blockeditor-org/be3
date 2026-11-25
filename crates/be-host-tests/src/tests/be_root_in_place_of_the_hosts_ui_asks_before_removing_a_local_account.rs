use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Asks to remove the account Dev from Manage accounts in the files' menu.
fn ask_to_remove_dev(host: &mut HostTest) {
    host.click("#account.options.Dev");
    host.click("MenuItem \"Remove account…\"");
    host.wait("AlertDialog \"Remove Dev?\"");
}

#[test]
fn be_root_in_place_of_the_hosts_ui_asks_before_removing_a_local_account() {
    // be-host's own UI asks before removing an account kept on this device alone, naming what
    // is lost (`removing_a_local_account_asks_first_and_names_what_is_lost`). Started again
    // with the root in place of its UI, the root asks the same: Keep keeps it, and Remove
    // removes it.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    ask_to_remove_dev(&mut host);
    host.crop("AlertDialog \"Remove Dev?\"");
    host.snapshot("the_host_asks_before_removing_a_local_account");
    host.click("Button \"Keep\"");
    host.gone("AlertDialog \"Remove Dev?\"");
    host.click("#account.open.Dev");
    host.wait("#file-tree.host-menu");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    ask_to_remove_dev(&mut host);
    host.crop("AlertDialog \"Remove Dev?\"");
    host.snapshot("be_root_asks_before_removing_a_local_account");
    host.click("Button \"Keep\"");
    host.gone("AlertDialog \"Remove Dev?\"");
    host.wait("#account.open.Dev");

    ask_to_remove_dev(&mut host);
    host.click("#host.remove-account.remove");
    host.gone("#account.open.Dev");
    assert!(host.shown("No accounts yet."));
    host.close();
}
