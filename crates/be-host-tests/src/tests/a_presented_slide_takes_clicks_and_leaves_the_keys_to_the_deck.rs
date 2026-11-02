use super::*;

#[test]
fn a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Presentation");
    for (page, shown) in [
        ("Title page", "Slide 1 of 1"),
        ("Regular page", "Slide 2 of 2"),
    ] {
        app.click("#presentation.add");
        app.wait("#picker.slides");
        app.click(&format!("MenuItem \"{page}\""));
        app.gone("#picker.slides");
        app.wait(shown);
    }
    app.click("#presentation.present");
    app.wait("#presentation.playback");
    assert!(
        !app.shown("#infinite-canvas.dock"),
        "a presented slide shows no tools"
    );
    app.wait("Label value=\"Header\"");
    let (x, y, width, height) = app.place("#infinite-canvas.canvas");
    let middle = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    app.run(&["click", &middle]);
    app.key("ArrowLeft");
    app.wait("Label value=\"Title\"");
    assert!(
        !app.shown("Label value=\"Header\""),
        "the arrow went to the deck, which went back a slide"
    );
    app.crop("#presentation.stage");
    app.snapshot("a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck");
    app.key("Escape");
    app.gone("#presentation.playback");
    app.wait("#infinite-canvas.dock");
    app.close();
}
