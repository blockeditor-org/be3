use super::*;

#[test]
fn a_presentation_menu_offers_new_slides_first() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    app.click("#presentation.add");
    app.wait("#picker.slides");
    assert!(
        !app.shown("MenuItem \"Text\""),
        "the common types wait behind Other blocks"
    );
    app.snapshot("a_presentation_menu_offers_new_slides_first");
    app.click("MenuItem \"Title page\"");
    app.gone("#picker.slides");
    app.wait("Slide 1 of 1");
    app.close();
}
