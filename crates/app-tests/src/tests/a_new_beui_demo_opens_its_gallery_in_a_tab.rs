use super::*;

#[test]
fn a_new_beui_demo_opens_its_gallery_in_a_tab() {
    let mut app = AppTest::start();
    app.click(ADD_TO_MY_FILES);
    choose_type(&mut app, "beui demo");
    app.wait("Pane \"beui demo\"");
    app.wait("Components");
    app.snapshot("a_new_beui_demo_opens_its_gallery_in_a_tab");
    app.close();
}
