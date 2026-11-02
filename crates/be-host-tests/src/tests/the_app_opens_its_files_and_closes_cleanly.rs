use super::*;

#[test]
fn the_app_opens_its_files_and_closes_cleanly() {
    let mut app = AppTest::start();
    app.wait("TreeItem \"Recently Deleted\"");
    app.snapshot("the_app_opens_with_its_files");
    app.close();
}
