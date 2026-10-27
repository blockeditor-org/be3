use super::*;

#[test]
fn the_window_sample_floats_beside_its_button_where_nothing_opens_windows() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Layering);
    reveal(&mut test, "demo.window.trigger");

    test.click("demo.window.trigger");
    test.frame(Vec::new());
    assert!(
        test.shown("demo.window.panel"),
        "the button opens the window, shown over the page as an overlay"
    );
    let trigger = test.rect_of("demo.window.trigger");
    let panel = test.rect_of("demo.window.panel");
    assert!(
        panel.top() > trigger.top() && panel.left() > trigger.left(),
        "it floats beside where the button was clicked: {panel:?} by {trigger:?}"
    );
    test.snapshot("window_sample_as_an_overlay");

    test.click("demo.window.trigger");
    test.frame(Vec::new());
    assert!(
        !test.shown("demo.window.panel"),
        "the button closes it again"
    );
}
