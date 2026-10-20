use super::*;

#[test]
fn linking_an_existing_block_browses_a_folder_at_a_time() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Folder");
    app.click(ADD_TO_MY_FILES);
    app.wait("Dialog \"Add block\"");
    app.click("\"Link existing\"");
    app.wait("Button \"My files\" #picker.link");
    app.crop("Dialog \"Add block\"");
    app.snapshot("linking_an_existing_block_browses_a_folder_at_a_time.top");
    app.click("Button \"My files\" #picker.link");
    app.wait("#picker.up");
    app.snapshot("linking_an_existing_block_browses_a_folder_at_a_time.inside");
    app.click("#picker.up");
    app.gone("#picker.up");
    app.click("#picker.cancel");
    app.gone("Dialog \"Add block\"");
    app.close();
}
