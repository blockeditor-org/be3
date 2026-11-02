use super::*;

#[test]
fn a_new_text_block_opens_in_a_tab_beside_the_files() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Text");
    host.click("#text.surface");
    host.type_text("Hello from a tab");
    host.wait("Hello from a tab");
    host.crop("#workspace.block");
    host.snapshot("a_new_text_block_opens_in_a_tab_beside_the_files");
    host.close();
}
