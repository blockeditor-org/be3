use super::*;

#[test]
fn going_back_from_the_add_pickers_location_page_grows_the_menu_again() {
    let mut host = HostTest::start(Options::default());
    open_add_to_my_files(&mut host);
    host.wait("#picker.import");
    let (_, _, _, full) = host.place("#picker.child");
    host.click("#picker.location");
    host.wait("#picker.location.back");
    let (_, _, _, short) = host.place("#picker.child");
    assert!(short < full, "the location page is shorter than the menu");

    host.click("#picker.location.back");
    host.wait("#picker.import");
    assert_eq!(
        host.place("#picker.child").3,
        full,
        "the menu takes its full height again"
    );
    assert!(
        host.tree()
            .lines()
            .any(|line| line.contains("#picker.import") && !line.contains("offscreen")),
        "its rows are laid out in that height before the pointer moves, not left in the location page's"
    );
    host.snapshot("going_back_from_the_add_pickers_location_page_grows_the_menu_again");
    host.key("Escape");
    host.gone("#picker.more");
    host.close();
}
