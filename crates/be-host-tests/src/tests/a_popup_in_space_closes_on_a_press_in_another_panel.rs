use super::*;

#[test]
fn a_popup_in_space_closes_on_a_press_in_another_panel() {
    let mut host = HostTest::start_in_space();
    host.wait(NEW);
    host.run(&["--into=space", "click", NEW]);
    host.run(&["--into=space", "click", NEW_IN_MY_FILES]);
    host.wait("#picker.more");
    host.snapshot("a_popup_in_space_closes_on_a_press_in_another_panel");

    host.run(&["--into=space", "click", "Tab \"Settings\""]);
    host.gone("#picker.more");
    assert!(
        host.shown("Tab \"Settings\" selected"),
        "the press that closes the add menu goes on to the tab it landed on"
    );
    host.close();
}
