use super::*;

#[test]
fn typing_after_a_new_session_opens_reaches_the_session() {
    let mut host = HostTest::start_desktop();
    host.click("#desktop.apps");
    host.click("#desktop.apps.sessions.new");
    host.wait("Pane \"Files\"");
    host.key("Tab");
    host.type_text("notes");
    host.wait("TextInput \"Filter files\" value=\"notes\" focused");
    host.close();
}
