use super::*;

#[test]
fn a_new_text_block_opens_in_a_tab_beside_the_files() {
    let mut app = AppTest::start();
    add_root_block(&mut app, "Text");
    app.wait("TreeItem \"Text\"");
    app.click("#text.surface");
    app.type_text("Hello from a tab");
    app.wait("Hello from a tab");
    app.crop("#workspace.block");
    app.snapshot("a_new_text_block_opens_in_a_tab_beside_the_files");
    app.close();
}
