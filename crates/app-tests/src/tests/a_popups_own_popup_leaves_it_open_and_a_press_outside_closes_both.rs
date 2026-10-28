use super::*;

#[test]
fn a_popups_own_popup_leaves_it_open_and_a_press_outside_closes_both() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.run(&["scroll", "#demo.catalog.Pickers", "1500"]);
    app.wait("#demo.catalog.Layering");
    app.run(&["a11y", "#demo.catalog.Layering", "click"]);
    app.wait("#demo.popup.trigger");
    app.click("#demo.popup.trigger");
    app.wait("#demo.popup.panel");
    app.click("#demo.popup.more");
    app.wait("#demo.popup.inner");

    app.click("#demo.popup.inner");
    assert!(
        app.shown("#demo.popup.panel"),
        "a press in a popup's own popup leaves the popup open"
    );
    app.snapshot("a_popups_own_popup_leaves_it_open_and_a_press_outside_closes_both");

    let (panel_x, panel_y, _, _) = app.place("#demo.popup.panel");
    app.run(&["click", &format!("{},{}", panel_x + 6.0, panel_y + 6.0)]);
    app.gone("#demo.popup.inner");
    assert!(
        app.shown("#demo.popup.panel"),
        "a press in the popup closes only its own popup"
    );

    app.click("#demo.popup.more");
    app.wait("#demo.popup.inner");
    app.click("#demo.catalog.Frames");
    app.gone("#demo.popup.panel");
    app.gone("#demo.popup.inner");
    assert!(
        app.shown("#demo.popup.trigger"),
        "the press that closes them goes no further, so the catalog keeps its page"
    );
    app.close();
}
