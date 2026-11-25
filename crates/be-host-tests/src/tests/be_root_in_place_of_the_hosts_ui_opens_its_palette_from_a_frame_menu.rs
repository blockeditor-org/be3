use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const DOCKING_MORE: &str = "#dock.joined:2/pages.menu";

#[test]
fn be_root_in_place_of_the_hosts_ui_opens_its_palette_from_a_frame_menu() {
    // be-host's own UI opens a beui demo, whose dock joins the workspace's; started again with
    // the root in place of its UI, the Command palette row of its Docking tab's menu opens the
    // one palette the root draws, as `a_joined_blocks_tab_opens_its_details_and_palette` does
    // under be-host's: the pick lets the demo's editor ask for it though its menu, not its
    // view, has the keyboard. It lists the demo's actions, its disabled Undo and Redo last.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    open_beui_demo(&mut host);
    let centre = |place: (f32, f32, f32, f32)| {
        format!(
            "{:.0},{:.0}",
            place.0 + place.2 / 2.0,
            place.1 + place.3 / 2.0
        )
    };
    let more = centre(host.place(DOCKING_MORE));
    host.click(DOCKING_MORE);
    let row = centre(host.place("MenuItem \"Command palette\""));
    host.key("Escape");
    host.restart_as_root(ROOT);
    host.run(&["settle"]);
    host.run(&["click", &more]);
    host.run(&["settle"]);
    host.run(&["click", &row]);
    host.run(&["settle"]);
    host.snapshot("be_root_opens_its_palette_from_a_frame_menu");
    host.close();
}
