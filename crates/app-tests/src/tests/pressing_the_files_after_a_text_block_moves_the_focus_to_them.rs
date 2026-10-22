use super::*;

#[test]
fn pressing_the_files_after_a_text_block_moves_the_focus_to_them() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Text");
    app.click("#text.surface");
    app.wait("MultilineTextInput focused");
    app.click("Pane \"Files\"");
    app.gone("MultilineTextInput focused");
    app.snapshot("pressing_the_files_after_a_text_block_moves_the_focus_to_them");
    app.close();
}
