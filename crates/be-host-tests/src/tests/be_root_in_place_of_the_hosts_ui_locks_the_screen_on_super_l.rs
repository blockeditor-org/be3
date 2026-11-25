use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_locks_the_screen_on_super_l() {
    // Under `--desktop` be-host runs the screen lock, which Super+L locks: its own UI shows its
    // lock screen. Started again with the root in place of its UI, the root asks the host to
    // lock on Super+L, and shows the host's lock (its user and power actions) on its own lock
    // screen, over the desktop shell.
    let mut host = HostTest::start(Options {
        desktop: true,
        previews: true,
        ..Options::default()
    });
    host.key("super+l");
    host.run(&["settle"]);
    host.snapshot("the_host_locks_the_screen_on_super_l");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.key("super+l");
    host.run(&["settle"]);
    host.snapshot("be_root_locks_the_screen_on_super_l");
    host.close();
}
