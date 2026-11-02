use super::*;

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn a_joined_blocks_tab_opens_its_details_and_palette() {
    let mut host = HostTest::start();
    open_beui_demo(&mut host);
    host.click(DOCKING_MORE);
    host.wait("MenuItem \"File details\"");
    host.click("MenuItem \"File details\"");
    host.wait("#workspace.details.rename");
    host.run(&["pause", "600"]);
    host.snapshot("a_joined_blocks_tab_opens_its_details");
    host.key("Escape");
    host.gone("#workspace.details.rename");
    host.run(&["pause", "600"]);

    host.click(DOCKING_MORE);
    host.click("MenuItem \"Command palette\"");
    host.gone("MenuItem \"Command palette\"");
    host.run(&["settle"]);
    host.snapshot("a_joined_blocks_tab_opens_its_palette");
    host.key("Escape");
    host.wait("Tab \"Docking\"");
    host.close();
}
