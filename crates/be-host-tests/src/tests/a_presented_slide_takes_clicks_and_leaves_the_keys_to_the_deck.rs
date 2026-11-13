use super::*;

#[test]
fn a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Presentation");
    for (page, shown) in [
        ("Title page", "Slide 1 of 1"),
        ("Regular page", "Slide 2 of 2"),
    ] {
        host.click("#presentation.add");
        host.wait("#picker.slides");
        host.click(&format!("MenuItem \"{page}\""));
        host.gone("#picker.slides");
        host.wait(shown);
    }
    host.click("#presentation.present");
    host.wait("#presentation.playback");
    assert!(
        !host.shown("#infinite-canvas.dock"),
        "a presented slide shows no tools"
    );
    host.wait("Label value=\"Header\"");
    let (x, y, width, height) = host.place("#infinite-canvas.canvas");
    let middle = format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0);
    host.run(&["click", &middle]);
    host.key("ArrowLeft");
    host.wait("Label value=\"Title\"");
    assert!(
        !host.shown("Label value=\"Header\""),
        "the arrow went to the deck, which went back a slide"
    );
    host.crop("#presentation.stage");
    host.snapshot("a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck");
    host.key("Escape");
    host.gone("#presentation.playback");
    host.wait("#infinite-canvas.dock");
    host.close();
}
