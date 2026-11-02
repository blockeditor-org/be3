use super::*;

#[test]
fn the_desktop_shell_opens_and_closes_cleanly() {
    let app = AppTest::start_desktop();
    app.close();
}
