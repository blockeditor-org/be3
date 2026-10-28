use super::*;

#[test]
fn a_popup_in_space_closes_on_a_press_in_another_panel() {
    let mut app = AppTest::start_in_space();
    app.wait(ADD_TO_MY_FILES);
    app.run(&["--into=space", "click", ADD_TO_MY_FILES]);
    app.wait("#picker.more");
    app.snapshot("a_popup_in_space_closes_on_a_press_in_another_panel");

    app.run(&["--into=space", "click", "Tab \"Settings\""]);
    app.gone("#picker.more");
    assert!(
        app.shown("Tab \"Files\" selected"),
        "the press that closes the add menu goes no further, so the Files tab stays selected"
    );
    app.close();
}
