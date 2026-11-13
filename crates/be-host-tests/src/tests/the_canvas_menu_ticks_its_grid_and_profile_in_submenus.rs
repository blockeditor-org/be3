use super::*;

const INSPECTOR_MENU: &str = "#dock.joined:2/inspector.menu";

#[test]
fn the_canvas_menu_ticks_its_grid_and_profile_in_submenus() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.wait("#infinite-canvas.canvas");
    host.click("#dock.editors.menu");
    host.click("#dock.editors.menu.canvas.view");
    host.click("#dock.editors.menu.canvas.grid-menu");
    host.wait("#dock.editors.menu.canvas.grid-medium");
    host.snapshot("the_canvas_menu_ticks_show_grid_and_its_medium_spacing");

    host.key("Escape");
    host.key("Escape");
    host.click("#dock.editors.menu.canvas.profile");
    host.wait("#dock.editors.menu.canvas.profile.vector");
    host.snapshot("the_canvas_menu_ticks_the_prototyping_profile");
    host.click("#dock.editors.menu.canvas.profile.vector");
    host.gone("#dock.editors.menu.canvas.profile.vector");

    host.click(INSPECTOR_MENU);
    host.click(&format!("{INSPECTOR_MENU}.canvas.profile"));
    host.wait(&format!("{INSPECTOR_MENU}.canvas.profile.vector"));
    host.snapshot("the_canvas_menu_ticks_the_vector_profile_once_picked");
    host.close();
}
