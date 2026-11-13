use super::*;

#[test]
fn closing_a_tab_shuts_its_editor_down() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    host.click("Button \"Close tab\"");
    host.gone("Pane \"Text\"");
    host.wait("No file open");
    host.close();
}
