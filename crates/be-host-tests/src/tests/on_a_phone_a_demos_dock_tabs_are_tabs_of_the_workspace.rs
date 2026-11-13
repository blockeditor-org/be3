use super::*;

#[test]
fn on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.run(&["resize", "390x800"]);
    host.run(&["settle"]);
    host.wait("#dock.back");
    host.click("#dock.switch");
    host.wait("#dock.switcher.tab.1");
    host.run(&["pause", "600"]);
    host.wait("Button \"Components\"");
    host.wait("Label value=\"Docking\"");
    host.snapshot("on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace");
    host.close();
}
