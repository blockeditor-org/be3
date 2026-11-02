use super::*;

#[test]
fn a_new_text_block_opens_in_a_tab_beside_the_files() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click("#text.surface");
    app.type_text("Hello from a tab");
    app.wait("Hello from a tab");
    app.crop("#workspace.block");
    app.snapshot("a_new_text_block_opens_in_a_tab_beside_the_files");
    app.close();
}
