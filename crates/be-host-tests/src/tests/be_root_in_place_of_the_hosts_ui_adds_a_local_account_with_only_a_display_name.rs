use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Opens Manage accounts from the files' menu and adds an account on this device, named Ada,
/// stopping before it is created.
fn name_a_local_account(host: &mut HostTest) {
    host.click("#file-tree.host-menu");
    host.click("#host.menu.accounts");
    host.click("Button \"Add account\"");
    host.click("#account.local");
    host.click("#account.name");
    host.type_text("Ada");
    let tree = host.tree();
    assert!(!tree.contains("Email address"));
    assert!(!tree.contains("Password"));
    host.crop("Dialog \"Create local account\"");
}

#[test]
fn be_root_in_place_of_the_hosts_ui_adds_a_local_account_with_only_a_display_name() {
    // be-host's own UI asks only for a display name for an account on this device
    // (`a_local_account_needs_only_a_display_name`). Started again with the root in place of
    // its UI, the root asks the same in the same dialog, and the account it creates opens on
    // its organizations with no recovery phrase to write down.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    name_a_local_account(&mut host);
    host.snapshot("the_host_asks_a_local_account_only_its_display_name");
    host.click("Button \"Cancel\"");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    name_a_local_account(&mut host);
    host.snapshot("be_root_asks_a_local_account_only_its_display_name");
    host.click("#account.confirm");
    host.wait("Button \"Personal\"");
    assert!(!host.tree().contains("Check that you saved them"));
    host.close();
}
