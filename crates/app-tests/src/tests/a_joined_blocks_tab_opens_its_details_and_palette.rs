use super::*;

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn a_joined_blocks_tab_opens_its_details_and_palette() {
    let mut app = AppTest::start();
    open_beui_demo(&mut app);
    app.click(DOCKING_MORE);
    app.wait("MenuItem \"File details\"");
    app.run(&["clock", "run"]);
    app.click("MenuItem \"File details\"");
    app.wait("#workspace.details.rename");
    app.run(&["clock", "stop"]);
    app.run(&["pause", "600"]);
    app.snapshot("a_joined_blocks_tab_opens_its_details");
    app.run(&["clock", "run"]);
    app.key("Escape");
    app.gone("#workspace.details.rename");
    app.run(&["clock", "stop"]);
    app.run(&["pause", "600"]);

    app.click(DOCKING_MORE);
    app.click("MenuItem \"Command palette\"");
    app.gone("MenuItem \"Command palette\"");
    app.run(&["settle"]);
    app.snapshot("a_joined_blocks_tab_opens_its_palette");
    app.key("Escape");
    app.wait("Tab \"Docking\"");
    app.close();
}
