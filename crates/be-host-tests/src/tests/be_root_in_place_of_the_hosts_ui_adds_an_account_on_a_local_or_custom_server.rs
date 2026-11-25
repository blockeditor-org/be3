use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens Add account from Manage accounts in the files' menu: where to keep it, with no
/// default server in a dev build, then another server's address.
fn add_an_account(host: &mut HostTest, painted: &str) {
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    assert!(!host.tree().contains("#account.default"));
    host.crop("Dialog \"Add account\"");
    host.snapshot(&format!("{painted}_chooses_where_an_account_lives"));
    host.click("#account.custom");
    host.wait("#account.server");
    assert!(!host.tree().contains("https://api.be3.pfg.pw"));
    host.crop("Dialog \"Connect to server\"");
    host.snapshot(&format!("{painted}_asks_a_remote_account_for_server_and_gateway"));
}

#[test]
fn be_root_in_place_of_the_hosts_ui_adds_an_account_on_a_local_or_custom_server() {
    // be-host's own UI asks where to keep an account it adds, then another server's address
    // and gateway token (`adding_an_account_chooses_a_local_or_custom_server`). Started again
    // with the root in place of its UI, the root asks the same in the same dialog.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_an_account(&mut host, "the_host");
    host.click("Button \"Cancel\"");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    add_an_account(&mut host, "be_root");
    host.close();
}
