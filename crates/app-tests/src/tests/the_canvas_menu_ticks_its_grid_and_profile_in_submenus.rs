use super::*;

#[test]
fn the_canvas_menu_ticks_its_grid_and_profile_in_submenus() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.wait("#infinite-canvas.canvas");
    app.click("#dock.editors.menu");
    app.click("#dock.editors.menu.canvas.view");
    app.click("#dock.editors.menu.canvas.grid-menu");
    app.wait("#dock.editors.menu.canvas.grid-medium");
    app.snapshot("the_canvas_menu_ticks_show_grid_and_its_medium_spacing");

    app.key("Escape");
    app.key("Escape");
    app.click("#dock.editors.menu.canvas.profile");
    app.wait("#dock.editors.menu.canvas.profile.vector");
    app.snapshot("the_canvas_menu_ticks_the_prototyping_profile");
    app.click("#dock.editors.menu.canvas.profile.vector");
    app.gone("#dock.editors.menu.canvas.profile.vector");

    app.click("#dock.editors.menu");
    app.click("#dock.editors.menu.canvas.profile");
    app.wait("#dock.editors.menu.canvas.profile.vector");
    app.snapshot("the_canvas_menu_ticks_the_vector_profile_once_picked");
    app.close();
}
