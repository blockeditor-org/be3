use super::*;

#[test]
fn a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.run(&["scroll", "#demo.catalog.Pickers", "1500"]);
    app.wait("#demo.catalog.Layering");
    app.run(&["a11y", "#demo.catalog.Layering", "click"]);
    app.wait("#demo.popup.trigger");
    let (x, y, width, height) = app.place("#demo.popup.trigger");
    app.click("#demo.popup.trigger");
    app.wait("#demo.popup.panel");

    let (panel_x, panel_y, _, _) = app.place("#demo.popup.panel");
    let asked = (x + width / 2.0 + 16.0, y + height / 2.0 + 16.0);
    assert!(
        (panel_x - asked.0).abs() < 2.0 && (panel_y - asked.1).abs() < 2.0,
        "the popup opens where the tab put it in its own window, {asked:?}, not at {:?}",
        (panel_x, panel_y)
    );
    app.snapshot("a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab");
    app.close();
}
