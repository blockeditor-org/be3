use super::*;

#[test]
fn the_window_sample_opens_a_window_of_its_own_where_a_host_opens_windows() {
    let (year, month, day) = TODAY;
    let document = alone_document(Date::new(year, month, day), Page::Layering);
    let mut test = DocumentTest::hosting_windows(document, WIDE);
    reveal(&mut test, "demo.window.trigger");

    test.click("demo.window.trigger");
    test.frame(Vec::new());
    test.frame(Vec::new());

    let opened = test.windows().open.clone();
    let [window] = opened.as_slice() else {
        panic!("the sample opens one window, not {opened:?}");
    };
    assert_eq!(
        window.size,
        Vec2::new(220.0, 120.0),
        "at the size it asks for"
    );
    let output = test.frame_in(window.id, Vec::new());
    assert_eq!(
        output
            .window(window.id)
            .and_then(|shown| shown.window.appearance.title.clone())
            .as_deref(),
        Some("A window"),
        "the window carries its title to the host"
    );
    test.snapshot_window("window_sample_in_a_window_of_its_own", window.id);
}
