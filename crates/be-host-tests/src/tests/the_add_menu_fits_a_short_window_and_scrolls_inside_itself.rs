use super::*;

#[test]
fn the_add_menu_fits_a_short_window_and_scrolls_inside_itself() {
    let mut host = HostTest::start();
    host.run(&["resize", "1100x220"]);
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    let (_, y, _, height) = host.place("#picker.location");
    assert!(
        y >= 0.0 && y + height <= 220.0,
        "the menu starts inside the window"
    );
    assert!(
        host.shown("#picker.import")
            && host
                .tree()
                .lines()
                .any(|line| line.contains("#picker.import") && line.contains("offscreen")),
        "the rows that do not fit wait below, inside the menu's own scroll"
    );
    host.snapshot("the_add_menu_fits_a_short_window_and_scrolls_inside_itself");
    let (x, y, width, height) = host.place("#picker.child");
    let inside = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    host.run(&["wheel", &inside, "10"]);
    assert!(
        host.tree()
            .lines()
            .any(|line| line.contains("#picker.import") && !line.contains("offscreen")),
        "scrolling the menu brings them up"
    );
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
