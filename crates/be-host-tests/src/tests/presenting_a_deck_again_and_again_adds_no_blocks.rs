use super::*;

#[test]
fn presenting_a_deck_again_and_again_adds_no_blocks() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Presentation");
    host.click("#presentation.add");
    host.wait("#picker.slides");
    host.click("MenuItem \"Title page\"");
    host.gone("#picker.slides");
    host.wait("Slide 1 of 1");
    host.run(&["settle"]);
    let before = host.count("TreeItem ");
    for _ in 0..3 {
        host.click("#presentation.present");
        host.wait("#presentation.playback");
        host.key("Escape");
        host.gone("#presentation.playback");
        host.wait("#presentation.toolbar");
        host.run(&["settle"]);
    }
    let tree = host.tree();
    assert_eq!(
        host.count("TreeItem "),
        before,
        "presenting and stopping adds nothing to the files:\n{tree}"
    );
    assert_eq!(host.count("TreeItem \"Presentation\""), 1, "{tree}");
    host.close();
}
