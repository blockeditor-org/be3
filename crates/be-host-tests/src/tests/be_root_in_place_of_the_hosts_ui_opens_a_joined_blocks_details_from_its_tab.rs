use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_a_joined_blocks_details_from_its_tab() {
    // be-host's own UI opens a beui demo, whose dock joins the workspace's; started again with
    // the root in place of its UI, the File details row of its Docking tab's menu asks for the
    // demo's details (the root's docks' details, sent as `child-details` to the workspace that
    // places it), which open as under be-host (`a_joined_blocks_tab_opens_its_details_and_palette`).
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    open_beui_demo(&mut host);
    host.restart_as_root(ROOT);
    host.wait("Tab \"Docking\"");
    host.run(&["settle"]);
    host.click(DOCKING_MORE);
    host.wait("MenuItem \"File details\"");
    host.click("MenuItem \"File details\"");
    host.wait("#workspace.details.rename");
    host.wait_still("#sheet.handle");
    host.snapshot("be_root_opens_a_joined_blocks_details_from_its_tab");
    host.close();
}
