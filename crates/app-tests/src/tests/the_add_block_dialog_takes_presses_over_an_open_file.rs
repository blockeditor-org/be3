use super::*;

#[test]
fn the_add_block_dialog_takes_presses_over_an_open_file() {
    let mut app = AppTest::start();
    add_root_block(&mut app, "Text");
    app.click("#file-tree.add-root");
    app.wait("Dialog \"Add block\"");
    app.click("Label value=\"Checklist\"");
    app.gone("Dialog \"Add block\"");
    app.wait("Pane \"Checklist\"");
    app.crop("#workspace.block");
    app.snapshot("the_add_block_dialog_takes_presses_over_an_open_file");
    app.close();
}
