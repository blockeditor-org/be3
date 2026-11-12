use super::*;

#[test]
fn typing_after_a_new_session_opens_reaches_the_session() {
    let mut host = HostTest::start(Options { desktop: true, ..Options::default() });
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.run(&["settle"]);
    host.key("Tab");
    let tree = host.tree();
    let focused: Vec<&str> = tree
        .lines()
        .filter(|line| line.contains(" focused"))
        .collect();
    assert!(
        focused
            .iter()
            .any(|line| line.contains("\"Filter files\"") || line.contains("Button \"New\"")),
        "Tab moves the focus inside the new session's files, not on the desktop: {focused:?}"
    );
    host.close();
}
