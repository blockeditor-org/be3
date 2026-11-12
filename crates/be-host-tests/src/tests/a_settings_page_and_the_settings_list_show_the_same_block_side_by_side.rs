use super::*;

/// Compare devices is the settings block in another editor, opened in a tab while the
/// settings list (the same block) stays in the side panel: each place keeps its own editor
/// at its own size.
#[test]
fn a_settings_page_and_the_settings_list_show_the_same_block_side_by_side() {
    let mut host = HostTest::start();
    host.click("Tab \"Settings\"");
    host.wait("#settings.devices");
    host.click("#settings.devices");
    host.wait("#devices.same");
    host.wait("selected focused #settings.devices");
    host.wait("#settings.display");
    host.run(&["settle"]);
    host.snapshot("a_settings_page_and_the_settings_list_show_the_same_block_side_by_side");
    host.close();
}
