use super::*;

#[test]
fn a_setting_opens_from_the_settings_tab_beside_the_files() {
    let mut app = AppTest::start();
    app.click("Tab \"Settings\"");
    app.wait("#settings.ui-settings");
    app.click("#settings.ui-settings");
    app.wait("#ui-settings.theme");
    app.wait("selected focused #settings.ui-settings");
    app.run(&["settle"]);
    app.snapshot("a_setting_opens_from_the_settings_tab_beside_the_files");
    app.close();
}
