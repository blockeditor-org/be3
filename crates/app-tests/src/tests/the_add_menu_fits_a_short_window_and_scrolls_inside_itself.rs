use super::*;

#[test]
fn the_add_menu_fits_a_short_window_and_scrolls_inside_itself() {
    let mut app = AppTest::start();
    app.run(&["resize", "1100x220"]);
    app.click(ADD_TO_MY_FILES);
    app.wait("#picker.more");
    let (_, y, _, height) = app.place("#picker.location");
    assert!(
        y >= 0.0 && y + height <= 220.0,
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
    let (x, y, width, height) = app.place("#picker.child");
    let inside = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    app.run(&["wheel", &inside, "10"]);
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
