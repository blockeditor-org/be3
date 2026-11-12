use super::*;

#[test]
fn a_popup_in_space_closes_on_a_press_in_another_panel() {
    let mut host = HostTest::start(Options {
        space: true,
        ..Options::default()
    });
    host.wait(NEW);
    host.run(&["--into=space", "click", NEW]);
    host.run(&["--into=space", "click", NEW_IN_MY_FILES]);
    host.wait("#picker.more");
    // The picker opens under the resting pointer, which does not always hover
    // its header row: the resting pointer is not handed on to a child popover
    // that replaces the menu it was pressed in. A move makes the painting the
    // same on every build server.
    host.run(&["--into=space", "move", "#picker.location"]);
    host.snapshot("a_popup_in_space_closes_on_a_press_in_another_panel");

    host.run(&["--into=space", "click", "Tab \"Settings\""]);
    host.gone("#picker.more");
    assert!(
        host.shown("Tab \"Settings\" selected"),
        "the press that closes the add menu goes on to the tab it landed on"
    );
    host.close();
}
