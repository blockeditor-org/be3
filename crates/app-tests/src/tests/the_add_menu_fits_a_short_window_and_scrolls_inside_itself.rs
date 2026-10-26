use super::*;

#[test]
fn the_add_menu_fits_a_short_window_and_scrolls_inside_itself() {
    let mut app = AppTest::start();
    app.run(&["screen", "resize", "1100x300"]);
    app.click(ADD_TO_MY_FILES);
    app.wait("#picker.more");
    let (_, y, _, height) = app.place("#picker.location");
    assert!(
        y >= 0.0 && y + height <= 300.0,
        "the menu starts inside the window"
    );
    assert!(
        app.shown("#picker.import")
            && app
                .tree()
                .lines()
                .any(|line| line.contains("#picker.import") && line.contains("offscreen")),
        "the rows that do not fit wait below, inside the menu's own scroll"
    );
    app.snapshot("the_add_menu_fits_a_short_window_and_scrolls_inside_itself");
    app.run(&["app", "wheel", "#picker.location", "3"]);
    assert!(
        app.tree()
            .lines()
            .any(|line| line.contains("#picker.import") && !line.contains("offscreen")),
        "scrolling the menu brings them up"
    );
    app.key("Escape");
    app.gone("#picker.more");
    app.close();
}
