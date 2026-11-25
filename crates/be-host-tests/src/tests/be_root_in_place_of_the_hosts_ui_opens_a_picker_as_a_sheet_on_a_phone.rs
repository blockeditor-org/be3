use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_a_picker_as_a_sheet_on_a_phone() {
    // On a phone, a tap on the canvas's Block button opens its block picker in a sheet the
    // canvas anchors to the bottom edge of its view. With the root in place of be-host's UI, the
    // root opens the picker it places there as a sheet, laid out for touch, as be-host does.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    host.restart_as_root(ROOT);
    // A phone the canvas's tool bar fits across, with its Block button where be-host lays it.
    host.run(&["resize", "520x900"]);
    host.run(&["settle"]);
    host.run(&["tap", "428,872"]);
    // A sheet slides up on the clock (guides/running/drive.md, Targets).
    host.pause(600);
    host.run(&["settle"]);
    host.snapshot("be_root_opens_a_picker_as_a_sheet_on_a_phone");
    host.close();
}
