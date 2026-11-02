use super::*;

#[test]
fn on_a_short_window_the_add_menu_rises_as_a_sheet_and_scrolls_inside_it() {
    let mut host = HostTest::start();
    host.run(&["resize", "1100x220"]);
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    host.run(&["pause", "600"]);
    let (_, y, _, height) = host.place("#picker.location");
    assert!(
        y >= 0.0 && y + height <= 220.0,
        "a short window is a phone on its side, so the menu rises as a sheet inside it"
    );
    let (_, import_y, _, _) = host.place("#picker.import");
    assert!(import_y > 220.0, "the rows that do not fit wait below");
    host.snapshot("on_a_short_window_the_add_menu_rises_as_a_sheet_and_scrolls_inside_it");
    let (x, y, width, height) = host.place("#picker.child");
    let inside = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    host.run(&["wheel", &inside, "10"]);
    host.run(&["pause", "600"]);
    let (_, import_y, _, import_height) = host.place("#picker.import");
    assert!(
        import_y + import_height <= 220.0,
        "scrolling the sheet brings them up: {import_y}"
    );
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
