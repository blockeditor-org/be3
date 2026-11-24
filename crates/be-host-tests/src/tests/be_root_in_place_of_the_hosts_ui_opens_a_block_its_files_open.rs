use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_a_block_its_files_open() {
    // be-host's own UI makes a canvas and closes its tab; started again with the root in place
    // of its UI, a press on the canvas in the Files asks the host to open it, which hands it to
    // the workspace, as be-host does, and the workspace opens it in a tab.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    host.click("Button \"Close tab\"");
    host.wait("No file open");
    let row = host.place("TreeItem \"Canvas\"");
    let row = format!("{:.0},{:.0}", row.0 + row.2 / 2.0, row.1 + row.3 / 2.0);
    host.restart_as_root(ROOT);
    // `settle` under the root can return while the Files are still arriving
    // (plans/host_as_platform_adapter.md, 4g7b3d): the stopped clock moved on lets them land
    // before the press, which a row not there yet would miss.
    host.pause(600);
    host.run(&["settle"]);
    host.run(&["click", &row]);
    host.run(&["settle"]);
    host.snapshot("be_root_opens_a_block_its_files_open");
    host.close();
}
