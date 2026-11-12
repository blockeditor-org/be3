use super::*;

#[test]
fn a_block_added_from_the_files_is_named_in_its_row() {
    let mut host = HostTest::start(Options::default());
    start_adding_to_my_files(&mut host, "Text");
    host.crop("#workspace.files");
    host.snapshot("a_block_added_from_the_files_is_named_in_its_row");
    host.type_text("Plans");
    host.key("Enter");
    host.wait("TreeItem \"Plans\"");
    host.wait("Tab \"Plans\"");
    host.close();
}
