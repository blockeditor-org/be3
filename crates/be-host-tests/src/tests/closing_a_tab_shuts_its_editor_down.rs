use super::*;

#[test]
fn closing_a_tab_shuts_its_editor_down() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click("Button \"Close tab\"");
    app.gone("Pane \"Text\"");
    app.wait("No file open");
    app.close();
}
