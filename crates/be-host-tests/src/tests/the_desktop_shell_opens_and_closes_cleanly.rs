use super::*;

#[test]
fn the_desktop_shell_opens_and_closes_cleanly() {
    let host = HostTest::start_desktop();
    host.close();
}
