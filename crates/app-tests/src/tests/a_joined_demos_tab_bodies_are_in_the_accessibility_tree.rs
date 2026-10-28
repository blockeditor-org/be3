use super::*;

#[test]
fn a_joined_demos_tab_bodies_are_in_the_accessibility_tree() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.wait("#demo.catalog.Docking");
    app.wait("Label value=\"The dock around this page\"");
    app.click("#demo.catalog.Text");
    app.wait("#demo.code.Text styles");
    app.wait("Label value=\"Body text for labels and values.\"");
    app.close();
}
