use super::*;

#[test]
fn going_back_from_the_add_pickers_location_page_grows_the_menu_again() {
    let mut app = AppTest::start();
    open_add_to_my_files(&mut app);
    app.wait("#picker.import");
    let (_, _, _, full) = app.place("#picker.child");
    app.click("#picker.location");
    app.wait("#picker.location.back");
    let (_, _, _, short) = app.place("#picker.child");
    assert!(short < full, "the location page is shorter than the menu");

    app.click("#picker.location.back");
    app.wait("#picker.import");
    assert_eq!(
        app.place("#picker.child").3,
        full,
        "the menu takes its full height again"
    );
    assert!(
        app.tree()
            .lines()
            .any(|line| line.contains("#picker.import") && !line.contains("offscreen")),
        "its rows are laid out in that height before the pointer moves, not left in the location page's"
    );
    app.snapshot("going_back_from_the_add_pickers_location_page_grows_the_menu_again");
    app.key("Escape");
    app.gone("#picker.more");
    app.close();
}
