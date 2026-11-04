use super::*;

#[test]
fn a_presentation_menu_offers_new_slides_first() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    assert!(
        !host.shown("MenuItem \"Text\""),
        "the common types wait behind Other blocks"
    );
    host.snapshot("a_presentation_menu_offers_new_slides_first");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("Slide 1 of 1");
    host.close();
}
