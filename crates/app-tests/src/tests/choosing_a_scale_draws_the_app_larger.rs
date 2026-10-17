use super::*;

#[test]
fn choosing_a_scale_draws_the_app_larger() {
    let mut app = AppTest::start();
    app.click("Tab \"Settings\"");
    app.click("#settings.ui-settings");
    app.click("#ui-settings.scale");
    app.key("Home");
    for _ in 0..5 {
        app.key("Down");
    }
    app.key("Enter");
    app.wait("#ui-settings.scale.reset");
    app.run(&["settle"]);
    app.snapshot("choosing_a_scale_draws_the_app_larger");
    app.close();
}
