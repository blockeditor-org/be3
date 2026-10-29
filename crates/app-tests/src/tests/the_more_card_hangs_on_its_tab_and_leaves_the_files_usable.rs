use super::*;

#[test]
fn the_more_card_hangs_on_its_tab_and_leaves_the_files_usable() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.block");
    app.click("#picker.more");
    app.wait("#picker.search");
    app.type_text("check");
    app.wait("Button \"Checklist\"");
    app.snapshot("the_more_card_hangs_on_its_tab_and_leaves_the_files_usable");

    app.click("TreeItem \"Canvas\"");
    app.gone("#picker.search");
    assert!(
        app.shown("TreeItem \"Canvas\" selected"),
        "the files take a press beside the card, which closes it"
    );
    app.click("#infinite-canvas.dock.block");
    app.click("#picker.more");
    app.wait("#picker.search");
    app.type_text("check");
    app.click("Button \"Checklist\"");
    app.gone("#picker.search");
    app.wait("GenericContainer #infinite-canvas.entity.");
    app.close();
}
