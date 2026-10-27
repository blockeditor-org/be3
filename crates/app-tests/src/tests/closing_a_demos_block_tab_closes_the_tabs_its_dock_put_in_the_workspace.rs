use super::*;

#[test]
fn closing_a_demos_block_tab_closes_the_tabs_its_dock_put_in_the_workspace() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.click("#dock.tab.2.close");
    app.gone("Tab \"Components\"");
    app.gone("Tab \"Docking\"");
    app.wait("No file open");
    app.close();
}
