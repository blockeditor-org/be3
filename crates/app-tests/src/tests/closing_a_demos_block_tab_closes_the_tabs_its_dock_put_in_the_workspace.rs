use super::*;

#[test]
fn closing_a_demos_block_tab_closes_the_tabs_its_dock_put_in_the_workspace() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    let tab = app.place("Tab \"beui demo\"");
    let close = format!("{:.0},{:.0}", tab.0 + tab.2 - 15.0, tab.1 + tab.3 * 0.5);
    app.run(&["click", &close]);
    app.gone("Tab \"Components\"");
    app.gone("Tab \"Docking\"");
    app.wait("No file open");
    app.close();
}
