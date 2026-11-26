use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

/// Kills the desktop shell from the host menu's Running Apps, which shows the session picker,
/// painted as `painted`; keeping the session brings the desktop back.
fn kill_the_desktop_shell(host: &mut HostTest, painted: &str) {
    host.click("#desktop.apps");
    host.click("#desktop.apps.menu");
    host.click("#host.menu.panel.Apps");
    host.click("#debug.apps.kill.be3.desktop");
    host.wait("#sessions.keep");
    host.gone("#desktop.apps");
    host.snapshot(painted);
    host.click("#sessions.keep");
    host.wait("#desktop.apps");
}

#[test]
fn be_root_in_place_of_the_hosts_ui_shows_the_session_picker_once_the_desktop_shell_is_killed() {
    // Killing be-host's desktop shell from Running Apps shows the session picker, and keeping
    // the session starts it again (`killing_the_desktop_shell_shows_the_session_picker`).
    // Started again with the root in place of its UI, the root lists the apps it runs, and
    // does the same when its desktop shell is killed.
    let options = Options {
        desktop: true,
        previews: true,
        ..Options::default()
    };
    let mut host = HostTest::start(options);
    kill_the_desktop_shell(
        &mut host,
        "the_host_shows_the_session_picker_once_its_shell_is_killed",
    );
    host.close();
    // A host of its own, which be-host's own UI never killed a shell on: a window its UI
    // opened is not shown when the root starts on the same data (the plan's 4g7e2).
    let mut host = HostTest::start(options);
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    kill_the_desktop_shell(
        &mut host,
        "be_root_shows_the_session_picker_once_its_shell_is_killed",
    );
    host.close();
}
