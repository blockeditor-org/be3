use super::*;

#[test]
fn on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.run(&["resize", "390x800"]);
    app.run(&["settle"]);
    app.wait("#dock.back");
    app.run(&["clock", "run"]);
    app.click("#dock.switch");
    app.wait("#dock.switcher.tab.1");
    app.run(&["pause", "600"]);
    app.run(&["clock", "stop"]);
    app.wait("Button \"Components\"");
    app.wait("Label value=\"Docking\"");
    app.snapshot("on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace");
    app.close();
}
