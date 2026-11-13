use super::*;

#[test]
fn a_setting_opens_from_the_settings_tab_beside_the_files() {
    let mut host = HostTest::start(Options::default());
    host.click("Tab \"Settings\"");
    host.wait("#settings.display");
    host.click("#settings.display");
    host.wait("#display.theme");
    host.wait("selected focused #settings.display");
    host.run(&["settle"]);
    host.snapshot("a_setting_opens_from_the_settings_tab_beside_the_files");
    host.close();
}
