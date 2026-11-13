use super::*;

#[test]
fn killing_the_desktop_shell_shows_the_session_picker() {
    let mut host = HostTest::start(Options {
        desktop: true,
        ..Options::default()
    });
    host.click("#desktop.apps");
    host.click("#desktop.apps.menu");
    host.click("#host.menu.panel.Apps");
    host.click("#debug.apps.kill.be3.desktop");
    host.wait("#sessions.keep");
    host.gone("#desktop.apps");
    host.snapshot("killing_the_desktop_shell_shows_the_session_picker");
    host.click("#sessions.keep");
    host.wait("#desktop.apps");
    host.close();
}
