use super::*;

/// The root be-host runs in place of its own UI (crates/be-root), staged as an app.
const ROOT: &str = "be3.root";

const MENU: &str = "#dock.editors.menu";
const INSPECTOR_MENU: &str = "#dock.joined:2/inspector.menu";

#[test]
fn be_root_in_place_of_the_hosts_ui_picks_from_a_joined_canvas_tab_menu() {
    // A canvas in the workspace's editors' area joins its dock to the workspace's, and its menu
    // is its tab's. Started again with the root in place of be-host's UI, a pick from that menu
    // (the root's docks' picks, sent as `menu-pick`) reaches the canvas, as under be-host
    // (`the_canvas_menu_ticks_its_grid_and_profile_in_submenus`): picking the vector profile
    // ticks it in the menu of the canvas's Inspector tab.
    let mut host = HostTest::start(Options {
        previews: true,
        ..Options::default()
    });
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    host.restart_as_root(ROOT);
    host.wait("#infinite-canvas.canvas");
    host.run(&["settle"]);
    host.click(MENU);
    host.click(&format!("{MENU}.canvas.profile"));
    host.wait(&format!("{MENU}.canvas.profile.vector"));
    host.click(&format!("{MENU}.canvas.profile.vector"));
    host.gone(&format!("{MENU}.canvas.profile.vector"));
    host.click(INSPECTOR_MENU);
    host.click(&format!("{INSPECTOR_MENU}.canvas.profile"));
    host.wait(&format!("{INSPECTOR_MENU}.canvas.profile.vector"));
    host.snapshot("be_root_ticks_the_vector_profile_once_picked");
    host.close();
}
