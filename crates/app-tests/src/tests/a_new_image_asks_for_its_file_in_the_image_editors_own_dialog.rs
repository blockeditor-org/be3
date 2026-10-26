use super::*;

#[test]
fn a_new_image_asks_for_its_file_in_the_image_editors_own_dialog() {
    let mut app = AppTest::start();
    app.click(ADD_TO_MY_FILES);
    choose_type(&mut app, "Image");
    app.wait("#creation.confirm");
    app.snapshot("a_new_image_asks_for_its_file_in_the_image_editors_own_dialog");

    app.key("Escape");
    app.gone("#creation.confirm");

    app.click(ADD_TO_MY_FILES);
    choose_type(&mut app, "Image");
    app.click("#creation.cancel");
    app.gone("#creation.confirm");
    app.close();
}
