use super::*;

#[test]
fn a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.run(&["scroll", "#demo.catalog.Pickers", "1500"]);
    host.wait("#demo.catalog.Layering");
    host.run(&["a11y", "#demo.catalog.Layering", "click"]);
    host.wait("#demo.popup.trigger");
    let (x, y, width, height) = host.place("#demo.popup.trigger");
    host.click("#demo.popup.trigger");
    host.wait("#demo.popup.panel");

    let (panel_x, panel_y, _, _) = host.place("#demo.popup.panel");
    let asked = (x + width / 2.0 + 16.0, y + height / 2.0 + 16.0);
    assert!(
        (panel_x - asked.0).abs() < 2.0 && (panel_y - asked.1).abs() < 2.0,
        "the popup opens where the tab put it in its own window, {asked:?}, not at {:?}",
        (panel_x, panel_y)
    );
    host.snapshot("a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab");
    host.close();
}
