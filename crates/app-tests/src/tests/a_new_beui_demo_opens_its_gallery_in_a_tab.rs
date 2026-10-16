use super::*;

#[test]
fn a_new_beui_demo_opens_its_gallery_in_a_tab() {
    let mut app = AppTest::start();
    app.click("#file-tree.add-root");
    pick_tile(
        &mut app,
        "#picker.tile.62657569-2d64-656d-6f2d-626c6f636b31/main",
    );
    app.wait("Pane \"beui demo\"");
    app.wait("Components");
    app.crop("#workspace.block");
    app.snapshot("a_new_beui_demo_opens_its_gallery_in_a_tab");
    app.close();
}
