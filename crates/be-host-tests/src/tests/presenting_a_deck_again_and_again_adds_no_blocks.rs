use super::*;

#[test]
fn presenting_a_deck_again_and_again_adds_no_blocks() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    app.click("#presentation.add");
    app.wait("#picker.slides");
    app.click("MenuItem \"Title page\"");
    app.gone("#picker.slides");
    app.wait("Slide 1 of 1");
    app.run(&["settle"]);
    let before = app.count("TreeItem ");
    for _ in 0..3 {
        app.click("#presentation.present");
        app.wait("#presentation.playback");
        app.key("Escape");
        app.gone("#presentation.playback");
        app.wait("#presentation.toolbar");
        app.run(&["settle"]);
    }
    let tree = app.tree();
    assert_eq!(
        app.count("TreeItem "),
        before,
        "presenting and stopping adds nothing to the files:\n{tree}"
    );
    assert_eq!(app.count("TreeItem \"Presentation\""), 1, "{tree}");
    app.close();
}
