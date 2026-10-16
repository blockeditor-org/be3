use super::*;

const PRESENTATION_TILE: &str = "#picker.tile.00007072-6573-656e-7461-74696f6e0001/main";
const TITLE_SLIDE_TILE: &str = "#picker.tile.00007072-6573-656e-7461-74696f6e0001/title-slide";

fn fullscreen(app: &mut AppTest) -> bool {
    app.run(&["state"]).contains("fullscreen yes")
}

#[test]
fn presenting_a_deck_fills_the_screen_until_escape() {
    let mut app = AppTest::start();
    app.click("#file-tree.add-root");
    app.wait("Dialog \"Add block\"");
    for _ in 0..50 {
        let tree = app.tree();
        if tree
            .lines()
            .any(|line| line.contains(PRESENTATION_TILE) && !line.contains("offscreen"))
        {
            break;
        }
        app.run(&["wheel", "Dialog \"Add block\"", "3"]);
    }
    app.click(PRESENTATION_TILE);
    app.wait("#presentation.add");
    app.click("#presentation.add");
    app.click(TITLE_SLIDE_TILE);
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
