use super::*;

fn fullscreen(host: &mut HostTest) -> bool {
    host.run(&["state"]).contains("fullscreen yes")
}

#[test]
fn presenting_a_deck_fills_the_screen_until_escape() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("Slide 1 of 1");
    host.click("#presentation.present");
    host.wait("#presentation.playback");
    host.pause(500);
    assert!(
        host.shown("#presentation.playback"),
        "the deck is still presented after it starts"
    );
    assert!(
        fullscreen(&mut host),
        "presenting makes the window fullscreen"
    );
    host.key("Escape");
    host.gone("#presentation.playback");
    assert!(!fullscreen(&mut host), "stopping leaves fullscreen");
    host.wait("#presentation.filmstrip");
    host.wait("#infinite-canvas.dock");
    host.run(&["settle"]);
    assert_eq!(
        host.count("#presentation.toolbar "),
        1,
        "the deck's dock is hosted again, toolbar and all"
    );
    assert!(
        host.shown("Slide 1 of 1"),
        "and its toolbar shows the deck again"
    );
    host.close();
}
