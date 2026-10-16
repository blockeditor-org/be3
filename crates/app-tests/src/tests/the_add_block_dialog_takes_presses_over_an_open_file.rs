use super::*;

#[test]
fn the_add_block_dialog_takes_presses_over_an_open_file() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click(ADD_TO_MY_FILES);
    app.wait("Dialog \"Add block\"");
    app.click("Label value=\"Checklist\"");
    app.gone("Dialog \"Add block\"");
    app.wait("Pane \"Checklist\"");
    app.crop("#workspace.block");
    app.snapshot("the_add_block_dialog_takes_presses_over_an_open_file");
    app.close();
}
