use super::*;

#[test]
fn pressing_the_files_after_a_text_block_moves_the_focus_to_them() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.wait("MultilineTextInput focused");
    host.click("Pane \"Files\"");
    host.gone("MultilineTextInput focused");
    host.snapshot("pressing_the_files_after_a_text_block_moves_the_focus_to_them");
    host.close();
}
