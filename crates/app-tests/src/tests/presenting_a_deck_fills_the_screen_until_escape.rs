use super::*;

fn fullscreen(app: &mut AppTest) -> bool {
    app.run(&["app", "state"]).contains("fullscreen yes")
}

#[test]
fn presenting_a_deck_fills_the_screen_until_escape() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    app.click("#presentation.add");
    app.wait("#picker.slides");
    app.click("MenuItem \"Title page\"");
    app.gone("#picker.slides");
    app.wait("Slide 1 of 1");
    app.click("#presentation.present");
    app.wait("#presentation.playback");
    app.pause(500);
    assert!(
        app.shown("#presentation.playback"),
        "the deck is still presented after it starts"
    );
    assert!(
        fullscreen(&mut app),
        "presenting makes the window fullscreen"
    );
    app.key("Escape");
    app.gone("#presentation.playback");
    assert!(!fullscreen(&mut app), "stopping leaves fullscreen");
    app.close();
}
