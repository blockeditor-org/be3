use super::*;

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn a_joined_blocks_tabs_carry_its_menu() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.click(DOCKING_MORE);
    app.wait("MenuItem \"File details\"");
    app.wait("MenuItem \"Referenced by (2)\"");
    app.snapshot("a_joined_blocks_tabs_carry_its_menu");
    app.click("MenuItem \"Rename\"");
    app.wait("\"Rename\"");
    app.wait("TextInput \"Name\"");
    app.key("Escape");
    app.gone("TextInput \"Name\"");
    app.close();
}
