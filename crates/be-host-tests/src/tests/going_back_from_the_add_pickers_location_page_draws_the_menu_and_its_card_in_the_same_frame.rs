use super::*;

const UNDRAWN: &str = "shows a view at a size it has yet to draw";

#[test]
fn going_back_from_the_add_pickers_location_page_draws_the_menu_and_its_card_in_the_same_frame() {
    let mut host = HostTest::start();
    open_add_to_my_files(&mut host);
    host.wait("#picker.import");
    host.click("#picker.location");
    host.wait("#picker.location.back");
    host.run(&["settle"]);
    let short = host.place("#picker.child");

    host.run(&["--no-settle", "key", "Left"]);
    let state = host.run(&["state"]);
    assert!(
        !state.contains(UNDRAWN),
        "the picker's menu and the card around it are drawn at the menu's new size in the frame that goes back:\n{state}"
    );
    let first = host.place("#picker.child");
    host.run(&["settle"]);
    let settled = host.place("#picker.child");
    assert!(
        settled.3 > short.3,
        "going back grows the menu, {short:?} to {settled:?}"
    );
    assert_eq!(
        first, settled,
        "the card takes the menu's size in the frame that goes back"
    );
    host.snapshot("going_back_from_the_add_pickers_location_page_draws_the_menu_and_its_card_in_the_same_frame");
    host.close();
}
