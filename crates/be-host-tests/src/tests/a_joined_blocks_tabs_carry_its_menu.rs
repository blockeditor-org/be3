use super::*;

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn a_joined_blocks_tabs_carry_its_menu() {
    let mut host = HostTest::start(Options::default());
    open_beui_demo(&mut host);
    host.click(DOCKING_MORE);
    host.wait("MenuItem \"File details\"");
    host.wait("MenuItem \"Referenced by (2)\"");
    host.snapshot("a_joined_blocks_tabs_carry_its_menu");
    host.click("MenuItem \"Rename\"");
    host.wait("\"Rename\"");
    host.wait("TextInput \"Name\"");
    host.key("Escape");
    host.gone("TextInput \"Name\"");
    host.close();
}
