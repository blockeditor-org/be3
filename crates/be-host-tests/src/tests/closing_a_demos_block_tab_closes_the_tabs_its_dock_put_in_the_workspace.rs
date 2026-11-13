use super::*;

#[test]
fn closing_a_demos_block_tab_closes_the_tabs_its_dock_put_in_the_workspace() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.click("#dock.tab.2.close");
    host.gone("Tab \"Components\"");
    host.gone("Tab \"Docking\"");
    host.wait("No file open");
    host.close();
}
